# Run a Loner-Strike server on a Linux machine (VPS)

A VPS is a small Linux machine you rent from a hosting company. Loner-Strike can set one up as a dedicated server for you over SSH: it installs Valve's server and Loner-Strike there and runs it as a service, so it keeps going when your computer is off.

You do four things by hand: rent the machine, make an SSH key, open the game ports in the provider's firewall, and fill in the form in the app. The app does the rest.

## What you need

- Loner-Strike on Windows, or on a Mac with the game in CrossOver (the app builds the server's plugins on your computer before uploading them).
- A machine with **Ubuntu or Debian** on an **x86 (AMD or Intel) CPU**. Ubuntu 22.04 or 24.04 is the safest choice.
- A **public IPv4 address**. IPv6-only machines don't work.
- About 15 to 30 minutes, most of it waiting for downloads.

ARM machines can't run the Counter-Strike server. That rules out Oracle's Ampere A1 shapes, AWS Graviton instances, Hetzner's CAX plans and a Raspberry Pi.

## 1. Choose a host and size

The game uses one CPU core, so a fast core matters more than many of them. Bots cost CPU.

| Players (bots included) | CPU | Memory |
|---|---|---|
| Up to about 10, a few bots | 1 vCPU | 1 GB |
| 20 to 32, with bots | 2 vCPU | 2 to 4 GB |

Disk: about 2 GB plus your maps. Any plan's disk is enough.

Where to rent one:

- **Free:** Oracle Cloud's Always Free tier includes small AMD machines (VM.Standard.E2.1.Micro), when Oracle has capacity. They suit a few friends and bots. Oracle's trial credits cover bigger machines for a while.
- **Cheap:** Hetzner, Vultr, DigitalOcean, OVH and Linode (Akamai) all have x86 plans at around $4 to $7 a month. Pick one close to where you and your friends live, since distance adds ping.

Shared web hosting can't run game servers. You need a VPS (also called a cloud server, droplet or instance).

## 2. Make an SSH key

The app logs in to the machine with an SSH key: a private key that stays on your computer and a public key (the `.pub` file) you give to the provider.

**Mac or Linux**, in Terminal:

```
mkdir -p ~/.ssh
ssh-keygen -t ed25519 -f ~/.ssh/loner-strike -N "" -C loner-strike
cat ~/.ssh/loner-strike.pub
```

**Windows 10 or 11**, in PowerShell:

```
mkdir $HOME\.ssh -Force
ssh-keygen -t ed25519 -f $HOME\.ssh\loner-strike -C loner-strike
Get-Content $HOME\.ssh\loner-strike.pub
```

On Windows, press Enter twice when it asks for a passphrase, to leave it empty. The key ends up in `C:\Users\<you>\.ssh\loner-strike`.

The last command prints one line starting with `ssh-ed25519`. That is the public key; copy it for the next step.

Notes:

- Keys with a passphrase aren't supported yet. Use a key without one for the server.
- A key you already have works if it's in OpenSSH format (ed25519 or RSA) and has no passphrase.
- A PuTTY key (`.ppk`) has to be converted first: open it in PuTTYgen and use Conversions > Export OpenSSH key.

## 3. Create the machine

The steps are much the same everywhere:

1. Create a new server, instance or droplet.
2. Pick **Ubuntu 24.04** or **22.04** as the image.
3. Pick an **x86 (AMD or Intel)** plan of the size you chose.
4. Make sure it gets a **public IPv4 address**.
5. Under SSH keys, add a new key and paste the line from `loner-strike.pub`.
6. Create it and wait until it's running. Note its **public IP address** and the **user name** it uses for SSH.

The user is `ubuntu` on Ubuntu images from Oracle and AWS, and `root` on Hetzner, DigitalOcean and Vultr. Either works; the user needs to be able to use `sudo` without a password, which is how these images come.

Logged in as `root`, the game server runs as root too. That works, but if you'd rather it ran as its own user, make one before adding the server (here called `cs`) and use it as the User in the app:

```
adduser --disabled-password --gecos "" cs
usermod -aG sudo cs
echo 'cs ALL=(ALL) NOPASSWD:ALL' > /etc/sudoers.d/cs
mkdir -p /home/cs/.ssh && cp ~/.ssh/authorized_keys /home/cs/.ssh/ && chown -R cs:cs /home/cs/.ssh
```

### Oracle Cloud

- Shape: under Change shape, pick **AMD** and **VM.Standard.E2.1.Micro** (Always Free) or a Flex shape if you have credits. Not Ampere.
- Image: Canonical Ubuntu 22.04 or 24.04. User: `ubuntu`.
- Networking: let it create a new virtual cloud network with a public subnet, and check that **Assign a public IPv4 address** is on. If the instance ends up without a public IP, open the instance, then Attached VNICs > the VNIC > IPv4 addresses, edit the private IP and pick **Ephemeral public IP**.
- SSH keys: Paste public keys.
- Firewall: the subnet's **security list** (Networking > Virtual cloud networks > your network > Security > the default security list > Add ingress rules).
- "Out of capacity" means Oracle has no free machines of that shape there right now. Try another availability domain, try again later, or use another shape.

### Hetzner Cloud

- Type: **Shared vCPU, x86** (the CX or CPX plans). Not the Arm (CAX) plans.
- Networking: keep **Public IPv4** turned on. It costs a little extra, but the game needs it.
- User: `root`.
- Firewall: Hetzner adds none by default. If you attach one (Firewalls in the project), add an inbound rule for UDP 27015-27020.

### DigitalOcean

- Create > Droplets, Ubuntu, any Basic plan (they're all x86).
- Authentication: SSH key > New SSH key, paste the public key.
- User: `root`.
- Firewall: none by default. If you use a cloud firewall (Networking > Firewalls), add an inbound rule: Custom, UDP, ports 27015-27020.

### Vultr

- Deploy > Cloud Compute, an x86 (Intel or AMD) plan, Ubuntu.
- SSH keys: add the public key at deploy.
- User: `root`.
- Firewall: none by default. If the server is in a firewall group (Network > Firewall), add an inbound rule: UDP, port range 27015-27020, source anywhere.

### AWS Lightsail

- Create instance > Linux/Unix > OS only > Ubuntu.
- SSH key pair: Change SSH key pair > Upload new, and pick `loner-strike.pub`.
- User: `ubuntu`.
- Firewall: the instance's **Networking** tab > IPv4 firewall > Add rule: Custom, UDP, port range 27015-27020.
- Its public IP changes when the instance is stopped and started. Attach a static IP (Networking > Create static IP) so the address stays the same.

## 4. Open UDP 27015-27020

Players connect over UDP. There are two firewalls in the way:

1. **The machine's own firewall** (iptables or ufw). The app opens the ports there during setup. You don't need to do anything.
2. **The provider's firewall**, called a security list, security group, firewall group or cloud firewall depending on the provider (see above). The app can't reach this one. Add an inbound rule:

   - Protocol: **UDP**
   - Ports: **27015-27020**
   - Source: anywhere (`0.0.0.0/0`)

Leave SSH (TCP 22) open as it is. The app uses it to set up and manage the server, and sends server commands through it, so nothing else needs opening.

## 5. Add it in Loner-Strike

1. Open **Servers > New server**.
2. Where it runs: **A Linux server (VPS)**.
3. **Address:** the machine's public IP address.
4. **User:** `ubuntu` or `root`, as above.
5. **SSH key:** click Browse and pick the private key, the file *without* `.pub` (`loner-strike`). On a Mac the `.ssh` folder is hidden in the file picker: press Cmd+Shift+G and type `~/.ssh`, or press Cmd+Shift+. to show hidden files.
6. Click **Check the server**. The app connects and shows the machine's fingerprint (`SHA256:...`).
   - If you've connected with `ssh` from this computer before, it says whether the fingerprint matches what your computer has on record.
   - If it's the first time, you can compare it with what the machine itself shows: run `ssh-keygen -lf /etc/ssh/ssh_host_ed25519_key.pub` in your provider's web console.
   - From then on the app refuses to connect if the fingerprint changes.
7. **Name:** what the server is called in the server browser.
8. **Game:** Counter-Strike 1.6 or Condition Zero.
9. **Set it up like:** **My game** copies your game's modes, settings, admins and maps. **Start fresh** gives the built-in modes and default settings.
10. **Who can join:**
    - **Listed on Steam** shows it in the Internet tab of Find Servers.
    - **Anyone with the address** keeps it out of the list; people join with the address you send them.
    - Either way, a join password keeps strangers out. Set it in the form or later on the server's Overview.
11. Click **Make the server**.

Setup takes 5 to 15 minutes. It:

- checks the machine is x86 and that the user can use sudo,
- installs the 32-bit libraries the server needs,
- opens UDP 27015-27020 in the machine's firewall,
- downloads Valve's dedicated server with SteamCMD,
- installs the Linux builds of Metamod-P, AMX Mod X and ReGameDLL,
- builds Loner-Strike on your computer and uploads it,
- installs a service called `lonerstrike-<name>` and starts it.

The server runs on UDP port 27015. It keeps running when the app is closed or your computer is off, and restarts by itself after a crash or a reboot of the machine.

## 6. Join and manage it

In **Servers**, each server has **Join**, **Start** and **Stop**, and **Open** for its own pages (modes, maps, bots, admins and the rest).

- **Join:** with a listed server, Join works with the game already open. With an unlisted one, Join restarts the game to connect if it's open.
- **Friends** join with `connect <address>:27015` in the game console, or through Find Servers if it's listed.
- **Console:** the server's Overview tab shows its console, and you can send server commands from there (`status`, `changelevel de_dust2`).
- Changes you save in the server's pages are uploaded to the machine. Player slots, who can join and the start options need a restart; the app says so.
- **Advanced** (on the Overview, under Settings): the port (27015 to 27019), the listen address, extra start options like `-pingboost 2`, and the console (RCON) password for tools like HLSW.
- **HLTV:** turn it on in the server's HLTV tab. It runs as its own service next to the server, on port 27020, and starts and stops with it. Spectators use `connect <address>:27020`. Recorded demos stay on the machine; **Get it** copies one into your game so you can watch it with `viewdemo`.

You can also manage it over SSH:

```
ssh -i ~/.ssh/loner-strike ubuntu@<address>
```

Then:

| Command | What it does |
|---|---|
| `lsctl list` | The Loner-Strike servers on the machine |
| `lsctl status` | Whether the server is running |
| `lsctl start` / `stop` / `restart` | Start, stop or restart it |
| `lsctl logs` | The last 100 lines of its console |

With more than one server, add its name: `lsctl restart friday-night-frags`. The files are in `~/loner-strike/servers/<name>`.

## Updating

- **The machine:** log in over SSH and run `sudo apt update && sudo apt upgrade`. If it asks for a reboot, run `sudo reboot`; the server starts again by itself.
- **Valve's server files** rarely change. Stop the server, then press **Update** in its Maintenance panel. SteamCMD checks Valve's files on the machine and the app sends Loner-Strike's files again; settings, admins, bans and stats stay.
- **Mods, plugins, maps and sounds:** changes you make in the app go to the machine by themselves within a few seconds, and only the files that changed are sent. Settings and admins take effect right away. New plugins, mods, maps and sounds load when the map changes; the server's page shows a **Restart now** button when there are some waiting.
- **Rebuild** in the Maintenance panel sets the server up again from scratch. It deletes everything in the server's folder on the machine first (settings, admins, bans, stats, added maps) and takes about as long as the first setup. Use it only when the server is broken.

## Troubleshooting

**"Failed to contact game server" or the server doesn't show up.**
The port is closed or the server is stopped. Check that the provider's firewall allows inbound UDP 27015-27020 (step 4), and that the server shows as Running in the app (or `lsctl status` says `active`).

**"Couldn't reach ... on port 22."**
The machine is off, the address is wrong, or the provider's firewall blocks SSH. On Oracle, check the instance has a public IP (see the Oracle notes).

**"The server didn't accept the key."**
The user name is wrong (`ubuntu` or `root`), or the public key wasn't added when the machine was created. Check that you picked the private key, not the `.pub` file.

**"That key has a passphrase."**
Passphrase keys aren't supported yet. Make a key without one (step 2) and add it to the machine: on most providers you can add a key from the web console, or log in another way and append the `.pub` line to `~/.ssh/authorized_keys`.

**"That's a PuTTY key (.ppk)."**
Export it from PuTTYgen with Conversions > Export OpenSSH key, and pick the exported file.

**"The server's identity has changed."**
The machine's host key is different from last time. If you rebuilt or replaced the machine, that's expected: remove the old entry with `ssh-keygen -R <address>`, then delete the server in the app and add it again. If you didn't, don't connect: something else may be answering at that address.

**"This machine is aarch64."**
It's an ARM machine. Make a new one with an x86 (AMD or Intel) plan.

**"The user ... can't use sudo without a password."**
Use `root`, or the image's default user (`ubuntu`).

**"Only Ubuntu and Debian are supported for now."**
Make the machine again with an Ubuntu image.

**Oracle: no public IP, or "out of capacity".**
See the Oracle notes in step 3.

**Setup stopped part way.**
The server shows as Not finished with two buttons. **Try again** carries on; every step is safe to run again. **Rebuild from scratch** deletes everything in the server's folder on the machine (settings, admins, bans, stats, added maps) and sets it up again like when you made it. Valve's download sometimes stops part way; Try again after a few minutes usually gets past it.

**The machine got a new IP address.**
The app keeps the old one. Give the machine a fixed address (Oracle reserved IP, Lightsail static IP) to avoid this. Otherwise delete the server in the app and add it again with the new address.

## Removing it

1. In the app, open the server and use **Delete the server** in Maintenance (or Delete in the server list). It stops the service, removes it and deletes the server's folder on the machine.
2. If you're done with the machine, delete or terminate it at your provider so it stops costing money. Deleting the server in the app doesn't do that.
