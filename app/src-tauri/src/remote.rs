//! Servers on a Linux machine somewhere else (a VPS): everything goes over SSH with the
//! player's own key, through a session the app keeps open while it's in use (ssh2: the
//! same on Mac and Windows, nothing needed on the computer). Nothing listens on the
//! server but the game and SSH.
//!
//! On the server (see SETUP_SH): Valve's server under ~/loner-strike/servers/<id>, the
//! Linux builds of Metamod-P, AMX Mod X and ReGameDLL, a systemd service per server
//! (lonerstrike-<id>: runs without the app, restarts on a crash or a reboot), and a small
//! `lsctl` helper for managing it by hand over SSH.

use serde::{Deserialize, Serialize};
use ssh2::Session;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Where and how to reach the machine.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct RemoteInfo {
    pub host: String,
    #[serde(default = "default_user")]
    pub user: String,
    /// The private key file on this computer (OpenSSH format, ed25519 or RSA)
    pub key: PathBuf,
    #[serde(default = "default_ssh_port")]
    pub ssh_port: u16,
    /// The machine's identity (SHA-256 of its host key), saved on first connect; a
    /// different one later means something is pretending to be it, so the app won't connect
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host_key: Option<String>,
}

fn default_user() -> String {
    "ubuntu".into()
}

fn default_ssh_port() -> u16 {
    22
}

impl RemoteInfo {
    fn label(&self) -> String {
        format!("{}@{}:{}", self.user, self.host, self.ssh_port)
    }
}

/// A server's folder on the machine (relative to the user's home).
pub fn server_dir(id: &str) -> String {
    format!("loner-strike/servers/{id}")
}

pub fn unit(id: &str) -> String {
    format!("lonerstrike-{id}")
}

// ---- the connection --------------------------------------------------------------------

// open sessions, with the fingerprint each one was verified against
static SESSIONS: Mutex<Option<HashMap<String, (Arc<Mutex<Session>>, String)>>> = Mutex::new(None);

fn connect(r: &RemoteInfo) -> Result<Session, String> {
    let addr = (r.host.as_str(), r.ssh_port)
        .to_socket_addrs()
        .map_err(|_| format!("Couldn't find {}. Check the address.", r.host))?
        .next()
        .ok_or_else(|| format!("Couldn't find {}.", r.host))?;
    let tcp = TcpStream::connect_timeout(&addr, Duration::from_secs(10))
        .map_err(|_| format!("Couldn't reach {} on port {}. Is the machine running, and is SSH open?", r.host, r.ssh_port))?;
    let mut s = Session::new().map_err(|e| e.to_string())?;
    s.set_tcp_stream(tcp);
    s.set_timeout(30_000);
    s.handshake().map_err(|e| format!("SSH didn't start ({e})."))?;
    check_identity(&s, r)?;
    if !r.key.is_file() {
        return Err(format!("The key file {} isn't there.", r.key.display()));
    }
    let first = std::fs::read_to_string(&r.key).unwrap_or_default();
    if first.starts_with("PuTTY-User-Key-File") {
        return Err("That's a PuTTY key (.ppk). Export it as an OpenSSH key in PuTTYgen (Conversions > Export OpenSSH key) and pick that.".into());
    }
    s.userauth_pubkey_file(&r.user, None, &r.key, None).map_err(|e| {
        if first.contains("ENCRYPTED") || e.message().contains("passphrase") {
            "That key has a passphrase, which isn't supported yet. Make a key without one for the server.".to_string()
        } else {
            format!("The server didn't accept the key for {} ({}). Check the user name and that this key was added to the server.", r.user, e.message())
        }
    })?;
    if !s.authenticated() {
        return Err("The server didn't accept the key.".into());
    }
    s.set_keepalive(true, 30);
    Ok(s)
}

/// The machine's fingerprint, written the way `ssh` shows it: SHA256:<base64, no padding>.
fn fingerprint(s: &Session) -> Option<String> {
    s.host_key_hash(ssh2::HashType::Sha256).map(|h| format!("SHA256:{}", base64_nopad(h)))
}

fn base64_nopad(bytes: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for c in bytes.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        for i in 0..=c.len() {
            out.push(T[((n >> (18 - 6 * i)) & 63) as usize] as char);
        }
    }
    out
}

/// Whether this computer's SSH has the machine on record (~/.ssh/known_hosts) with this key.
fn in_known_hosts(s: &Session, r: &RemoteInfo) -> bool {
    let (Some((key, _)), Some(home)) = (s.host_key(), dirs::home_dir()) else { return false };
    let Ok(mut kh) = s.known_hosts() else { return false };
    kh.read_file(&home.join(".ssh/known_hosts"), ssh2::KnownHostFileKind::OpenSSH).is_ok() && matches!(kh.check_port(&r.host, r.ssh_port, key), ssh2::CheckResult::Match)
}

/// A first look at a machine, before setting anything up: its fingerprint (for the
/// player to confirm), whether this computer's SSH already knows it, and its CPU type.
/// Uses a connection of its own that isn't kept, since the identity isn't confirmed yet.
pub fn probe(r: &RemoteInfo) -> Result<(String, bool, String), String> {
    let mut first = r.clone();
    first.host_key = None;
    let s = connect(&first)?;
    let fp = fingerprint(&s).ok_or("The server didn't identify itself.")?;
    let known = in_known_hosts(&s, r);
    let mut arch = String::new();
    let mut ch = s.channel_session().map_err(|e| e.to_string())?;
    ch.exec("uname -m").map_err(|e| e.to_string())?;
    let _ = ch.read_to_string(&mut arch);
    let _ = ch.wait_close();
    Ok((fp, known, arch.trim().to_string()))
}

/// Is this really the machine? Its key must match what this computer's SSH has on
/// record for it (~/.ssh/known_hosts) and what the app saved the first time.
fn check_identity(s: &Session, r: &RemoteInfo) -> Result<(), String> {
    const CHANGED: &str = "The server's identity has changed since this computer last connected, so the app won't connect. If you rebuilt the machine, remove its line from ~/.ssh/known_hosts and add the server again; otherwise someone may be intercepting the connection.";
    let (key, _) = s.host_key().ok_or("The server didn't identify itself.")?;
    if let (Some(home), Ok(mut kh)) = (dirs::home_dir(), s.known_hosts()) {
        if kh.read_file(&home.join(".ssh/known_hosts"), ssh2::KnownHostFileKind::OpenSSH).is_ok() {
            if let ssh2::CheckResult::Mismatch = kh.check_port(&r.host, r.ssh_port, key) {
                return Err(CHANGED.into());
            }
        }
    }
    if let Some(expected) = &r.host_key {
        if fingerprint(s).as_deref() != Some(expected.as_str()) {
            return Err(CHANGED.into());
        }
    }
    Ok(())
}



/// The open session for a machine (opened on first use, reopened if it dropped).
/// Only for a machine whose identity was confirmed (r.host_key): the probe before
/// setup uses its own connection that's never kept.
fn session(r: &RemoteInfo) -> Result<Arc<Mutex<Session>>, String> {
    let expected = r.host_key.clone().ok_or("The server's identity hasn't been confirmed.")?;
    let mut all = SESSIONS.lock().unwrap();
    let all = all.get_or_insert_with(HashMap::new);
    if let Some((s, fp)) = all.get(&r.label()) {
        if *fp == expected {
            return Ok(s.clone());
        }
    }
    // connect() checks the identity against r.host_key
    let s = connect(r)?;
    let fp = fingerprint(&s).ok_or("The server didn't identify itself.")?;
    let s = Arc::new(Mutex::new(s));
    all.insert(r.label(), (s.clone(), fp));
    Ok(s)
}

fn drop_session(r: &RemoteInfo) {
    if let Some(all) = SESSIONS.lock().unwrap().as_mut() {
        all.remove(&r.label());
    }
}

/// Runs `f` on the session, once more on a fresh one if the first try failed on the
/// connection (it may have dropped while idle).
fn with_session<T>(r: &RemoteInfo, f: impl Fn(&Session) -> Result<T, ssh2::Error>) -> Result<T, String> {
    for attempt in 0..2 {
        let s = session(r)?;
        let s = s.lock().unwrap();
        match f(&s) {
            Ok(v) => return Ok(v),
            Err(e) if attempt == 0 => {
                drop(s);
                drop_session(r);
                let _ = e;
            }
            Err(e) => return Err(format!("The connection to {} failed ({e}).", r.host)),
        }
    }
    unreachable!()
}

/// Runs a shell command; its exit code and output (stderr folded in).
pub fn exec(r: &RemoteInfo, cmd: &str) -> Result<(i32, String), String> {
    with_session(r, |s| {
        s.set_timeout(120_000);
        let mut ch = s.channel_session()?;
        ch.exec(&format!("{cmd} 2>&1"))?;
        // console text is Latin-1 (accented player names): bytes, not UTF-8
        let mut out = Vec::new();
        let _ = ch.read_to_end(&mut out);
        ch.wait_close()?;
        Ok((ch.exit_status()?, crate::rcon::latin1(&out)))
    })
}

/// Runs a command with `input` on its stdin, passing its output on line by line (long jobs).
pub fn exec_stream(r: &RemoteInfo, cmd: &str, input: &[u8], mut line: impl FnMut(&str)) -> Result<i32, String> {
    let s = session(r)?;
    let s = s.lock().unwrap();
    // setup can be quiet for a while (Valve's download), but not forever: a machine that
    // stops answering ends the job instead of holding the connection
    s.set_timeout(15 * 60 * 1000);
    let mut run = || -> Result<i32, ssh2::Error> {
        let mut ch = s.channel_session()?;
        ch.exec(&format!("{cmd} 2>&1"))?;
        if !input.is_empty() {
            ch.write_all(input).map_err(|e| ssh2::Error::new(ssh2::ErrorCode::Session(-7), Box::leak(e.to_string().into_boxed_str())))?;
        }
        ch.send_eof()?;
        // read until the command has really finished (a quiet spell isn't the end),
        // passing complete lines on as they come
        let mut pending: Vec<u8> = Vec::new();
        let mut chunk = [0u8; 8192];
        let mut quiet_since = std::time::Instant::now();
        loop {
            match ch.read(&mut chunk) {
                Ok(0) if ch.eof() => break,
                Ok(0) => std::thread::sleep(Duration::from_millis(50)),
                Ok(n) => {
                    quiet_since = std::time::Instant::now();
                    pending.extend_from_slice(&chunk[..n]);
                    while let Some(i) = pending.iter().position(|&b| b == b'\n') {
                        let l: Vec<u8> = pending.drain(..=i).collect();
                        line(String::from_utf8_lossy(&l).trim_end());
                    }
                }
                Err(e) if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut | std::io::ErrorKind::Interrupted) => {}
                Err(e) => return Err(ssh2::Error::new(ssh2::ErrorCode::Session(-7), Box::leak(e.to_string().into_boxed_str()))),
            }
            // (the end is a read of nothing once the other side has finished, so lines
            // still buffered when it finished are read first)
            // nothing at all for 15 minutes: the machine has stopped answering
            if quiet_since.elapsed() > Duration::from_secs(15 * 60) {
                return Err(ssh2::Error::new(ssh2::ErrorCode::Session(-9), "no answer for 15 minutes"));
            }
        }
        if !pending.is_empty() {
            line(String::from_utf8_lossy(&pending).trim_end());
        }
        ch.wait_close()?;
        ch.exit_status()
    };
    let r2 = run();
    s.set_timeout(30_000);
    drop(s);
    r2.map_err(|e| {
        drop_session(r);
        format!("The connection to {} dropped ({e}).", r.host)
    })
}

pub fn read_file(r: &RemoteInfo, path: &str) -> Result<Vec<u8>, String> {
    with_session(r, |s| {
        let sftp = s.sftp()?;
        let mut f = sftp.open(Path::new(path))?;
        let mut v = Vec::new();
        let _ = f.read_to_end(&mut v);
        Ok(v)
    })
}

/// Uploads a folder (the files `keep` lets through) into `to` on the machine, as one
/// compressed archive unpacked there.
pub fn upload_dir(r: &RemoteInfo, from: &Path, to: &str, keep: &dyn Fn(&Path) -> bool) -> Result<u64, String> {
    let mut bytes = Vec::new();
    let mut count = 0u64;
    {
        let gz = flate2::write::GzEncoder::new(&mut bytes, flate2::Compression::fast());
        let mut tar = tar::Builder::new(gz);
        for file in crate::archive::walk(from) {
            let Ok(rel) = file.strip_prefix(from) else { continue };
            if !keep(rel) || !file.is_file() {
                continue;
            }
            let name = rel.to_string_lossy().replace('\\', "/");
            let mut data = std::fs::read(&file).map_err(|e| format!("Couldn't read {name} ({e})."))?;
            // Linux reads a Windows line ending's CR as part of the line (in AMX Mod X's
            // configs a path ending in CR is a folder that doesn't exist): text goes up
            // with Linux line endings
            let lower = name.to_lowercase();
            if [".ini", ".cfg", ".txt", ".games"].iter().any(|x| lower.ends_with(x)) {
                data = lf(&data);
            }
            let mut h = tar::Header::new_gnu();
            h.set_size(data.len() as u64);
            h.set_mode(0o644);
            h.set_mtime(std::fs::metadata(&file).ok().and_then(|m| m.modified().ok()).and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_secs()).unwrap_or(0));
            h.set_cksum();
            tar.append_data(&mut h, &name, &data[..]).map_err(|e| format!("Couldn't pack {name} ({e})."))?;
            count += 1;
        }
        tar.into_inner().and_then(|gz| gz.finish()).map_err(|e| e.to_string())?;
    }
    let (code, out) = {
        let mut last = String::new();
        // settings (the panel's RCON password, the join password) readable by this user only
        let code = exec_stream(
            r,
            &format!("mkdir -p {to} && tar xzf - -C {to} && chmod -R go-rwx {to}/addons/amxmodx/data 2>/dev/null; true", to = shell_quote(to)),
            &bytes,
            |l| last = l.to_string(),
        )?;
        (code, last)
    };
    if code != 0 {
        return Err(format!("Unpacking on the server failed ({out})."));
    }
    Ok(count)
}

/// Text with Windows line endings (CR LF) turned into Linux ones (LF).
fn lf(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len());
    for (i, &b) in data.iter().enumerate() {
        if b == b'\r' && data.get(i + 1) == Some(&b'\n') {
            continue;
        }
        out.push(b);
    }
    out
}

/// 'text' for a shell command line.
pub fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

// ---- what runs on the machine --------------------------------------------------------

/// Sets up the machine and the server (safe to run again): the 32-bit libraries the
/// server needs, the game ports in the machine's own firewall, SteamCMD and Valve's
/// server, then Metamod-P, AMX Mod X and ReGameDLL (Linux builds, checked by sha256).
/// Progress lines start with @@STEP; a failure with @@FAIL.
pub const SETUP_SH: &str = r#"set -eu
step() { echo "@@STEP $1"; }
fail() { echo "@@FAIL $*"; exit 3; }
BASE="$HOME/loner-strike"; SRV="$HOME/$SRVDIR"; G="$GAME"
[ "$(uname -m)" = x86_64 ] || fail "This machine is $(uname -m). The Counter-Strike server only runs on x86 (AMD or Intel) machines."
sudo -n true 2>/dev/null || fail "The user $(whoami) can't use sudo without a password, which setup needs."

step packages
export DEBIAN_FRONTEND=noninteractive
command -v apt-get >/dev/null || fail "Only Ubuntu and Debian are supported for now."
sudo dpkg --add-architecture i386
sudo apt-get update -qq >/dev/null
sudo apt-get install -y -qq lib32gcc-s1 lib32stdc++6 libc6-i386 curl ca-certificates unzip xz-utils tar python3 >/dev/null

step firewall
if command -v iptables >/dev/null; then
  if ! sudo iptables -C INPUT -p udp --dport 27015:27020 -j ACCEPT 2>/dev/null; then
    # above a final REJECT rule if there is one (Oracle's images have one)
    pos=$(sudo iptables -L INPUT --line-numbers -n | awk '$2=="REJECT" {print $1; exit}')
    if [ -n "$pos" ]; then sudo iptables -I INPUT "$pos" -p udp --dport 27015:27020 -j ACCEPT
    else sudo iptables -A INPUT -p udp --dport 27015:27020 -j ACCEPT; fi
  fi
  if command -v netfilter-persistent >/dev/null; then sudo netfilter-persistent save >/dev/null 2>&1 || true
  elif [ -d /etc/iptables ]; then sudo sh -c 'iptables-save > /etc/iptables/rules.v4'; fi
fi
if command -v ufw >/dev/null && sudo ufw status | grep -q "Status: active"; then sudo ufw allow 27015:27020/udp >/dev/null; fi

step steamcmd
mkdir -p "$BASE/steamcmd"; cd "$BASE/steamcmd"
[ -x steamcmd.sh ] || curl -fsSL --proto '=https' https://steamcdn-a.akamaihd.net/client/installer/steamcmd_linux.tar.gz | tar xz

step server
mkdir -p "$SRV"
MOD=""; [ "$G" = czero ] && MOD="+app_set_config 90 mod czero"
# Valve's HLDS download often stops part way; running it again carries on
for i in 1 2 3 4 5; do
  ./steamcmd.sh +force_install_dir "$SRV" +login anonymous $MOD +app_update 90 validate +quit | grep --line-buffered -E "Update state|Success|ERROR" || true
  [ -f "$SRV/hlds_linux" ] && [ -f "$SRV/$G/liblist.gam" ] && break
done
[ -f "$SRV/hlds_linux" ] && [ -f "$SRV/$G/liblist.gam" ] || fail "Valve's download didn't finish. Try again in a while."
mkdir -p "$HOME/.steam/sdk32"
ln -sf "$BASE/steamcmd/linux32/steamclient.so" "$HOME/.steam/sdk32/steamclient.so"

step foundation
cd "$SRV/$G"; TMP=$(mktemp -d)
fetch() { curl -fsSL --proto '=https' "$1" -o "$TMP/$2" || fail "Couldn't download $2."; echo "$3  $TMP/$2" | sha256sum -c --quiet - || fail "$2 didn't match its checksum."; }
fetch "$MM_URL" mm.tar.xz "$MM_SHA"; tar xJf "$TMP/mm.tar.xz" -C .
fetch "$AMXX_URL" amxx.tar.gz "$AMXX_SHA"; tar xzf "$TMP/amxx.tar.gz" -C .
fetch "$AMXXCS_URL" amxxcs.tar.gz "$AMXXCS_SHA"; tar xzf "$TMP/amxxcs.tar.gz" -C .
fetch "$RG_URL" rg.zip "$RG_SHA"; unzip -o -q "$TMP/rg.zip" "bin/linux32/cstrike/*" -d "$TMP/rg"; cp -rf "$TMP/rg/bin/linux32/cstrike/." .
mkdir -p addons/metamod
echo "linux addons/amxmodx/dlls/amxmodx_mm_i386.so" > addons/metamod/plugins.ini
if grep -q '^gamedll_linux' liblist.gam; then sed -i 's|^gamedll_linux.*|gamedll_linux "addons/metamod/dlls/metamod.so"|' liblist.gam
else echo 'gamedll_linux "addons/metamod/dlls/metamod.so"' >> liblist.gam; fi
rm -rf "$TMP"
step ready
"#;

/// Linux builds of the foundation, the same versions as setup/foundation.rs's Windows ones.
pub const FOUNDATION_ENV: &[(&str, &str)] = &[
    ("MM_URL", "https://github.com/Bots-United/metamod-p/releases/download/v1.21p109/metamod-p-v1.21p109-linux_ubuntu2404.tar.xz"),
    ("MM_SHA", "bea62478a9d50023820188bda3893c720e7e7b1b01570292c747c5ef71829365"),
    ("AMXX_URL", "https://www.amxmodx.org/amxxdrop/1.10/amxmodx-1.10.0-git5487-base-linux.tar.gz"),
    ("AMXX_SHA", "d82d1d65c907f358c48bdebc07e20084e7e8c4f4d0e95af275e8b43f32815c7e"),
    ("AMXXCS_URL", "https://www.amxmodx.org/amxxdrop/1.10/amxmodx-1.10.0-git5487-cstrike-linux.tar.gz"),
    ("AMXXCS_SHA", "6a7541a213ef71c47c1348b3457722e95669c0135c60102d5cb5a876341aa808"),
    ("RG_URL", "https://github.com/rehlds/ReGameDLL_CS/releases/download/5.30.0.814/regamedll-bin-5.30.0.814.zip"),
    ("RG_SHA", "457f5c96a4d10280fcad47f106cbd5be86363a47df1ccab91c514e9d1bc6fd18"),
];

/// The systemd service for one server, and the `lsctl` helper for managing servers by hand.
pub fn service_sh(id: &str, game: &str, args: &[String]) -> String {
    let unit = unit(id);
    let dir = server_dir(id);
    // (the game's command line splits words at "-", so file names here have none)
    // all addresses: left to itself the server picks the name entry's 127.0.1.1 and
    // doesn't answer from outside; and the panel's RCON password from a file only this
    // user can read (the plugin sets it too, but the console shouldn't depend on it)
    let mut args = args.to_vec();
    args.extend(["+ip".into(), "0.0.0.0".into(), "+exec".into(), "lsrcon.cfg".into()]);
    let args = args.iter().map(|a| shell_quote(a)).collect::<Vec<_>>().join(" ");
    format!(
        r#"set -eu
SRV="$HOME/{dir}"
PW="$SRV/{game}/addons/amxmodx/data/gamemodes/panel_rcon.txt"
if [ -f "$PW" ]; then
  umask 077
  printf 'rcon_password "%s"\n' "$(tr -d '\r\n"' < "$PW")" > "$SRV/{game}/lsrcon.cfg"
fi
sudo tee /etc/systemd/system/{unit}.service >/dev/null <<EOF
[Unit]
Description=Loner-Strike server {id}
After=network-online.target
Wants=network-online.target

[Service]
User=$(whoami)
WorkingDirectory=$SRV
Environment=LD_LIBRARY_PATH=$SRV
ExecStart=$SRV/hlds_linux {args}
Restart=always
RestartSec=5

[Install]
WantedBy=multi-user.target
EOF
sudo tee /usr/local/bin/lsctl >/dev/null <<'EOF'
#!/bin/sh
# lsctl: Loner-Strike servers on this machine.  lsctl list | status | start | stop | restart | logs [server]
cmd=${{1:-help}}; id=${{2:-}}
units() {{ systemctl list-unit-files 'lonerstrike-*' --no-legend 2>/dev/null | awk '{{print $1}}' | sed 's/\.service$//; s/^lonerstrike-//'; }}
[ -z "$id" ] && id=$(units | head -n 1)
case "$cmd" in
  list) units ;;
  status) systemctl is-active "lonerstrike-$id" ;;
  start|stop|restart) sudo systemctl "$cmd" "lonerstrike-$id" ;;
  logs) tail -n "${{3:-100}}" "$HOME/loner-strike/servers/$id/"*/qconsole.log ;;
  *) echo "usage: lsctl list | status | start | stop | restart | logs [server] [lines]" ;;
esac
EOF
sudo chmod 755 /usr/local/bin/lsctl
sudo tee /usr/local/bin/lsrcon >/dev/null <<'LSRCON'
#!/usr/bin/env python3
# lsrcon <port> <password file> <command>: a server command to a game on this machine,
# for the Loner-Strike app (it sends commands over SSH, so the password stays here)
import socket, sys
port, pw = int(sys.argv[1]), open(sys.argv[2]).read().strip()
P = b"\xff\xff\xff\xff"
s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
s.settimeout(1.5)
try:
    s.sendto(P + b"challenge rcon\n", ("127.0.0.1", port))
    ch = s.recv(65535)[4:].split()[2]
    s.sendto(P + b"rcon " + ch + b' "' + pw.encode() + b'" ' + " ".join(sys.argv[3:]).encode("latin-1") + b"\n", ("127.0.0.1", port))
except (socket.timeout, OSError, IndexError):
    sys.exit(2)
singles, splits = [], {{}}
s.settimeout(0.4)
while True:
    try:
        d = s.recv(65535)
    except socket.timeout:
        break
    if len(d) > 9 and d[:4] == b"\xfe\xff\xff\xff":
        splits.setdefault(d[4:8], []).append((d[8] >> 4, d[9:]))
    else:
        singles.append(d)
for parts in splits.values():
    singles.append(b"".join(p for _, p in sorted(parts)))
sys.stdout.buffer.write(b"".join(d[5:] if d[:4] == P and len(d) > 5 else d for d in singles))
LSRCON
sudo chmod 755 /usr/local/bin/lsrcon
sudo systemctl daemon-reload
sudo systemctl enable {unit} >/dev/null 2>&1
echo "@@STEP ready"
"#
    )
}

/// Is the server's service running, and for how many seconds?
pub fn active(r: &RemoteInfo, id: &str) -> (bool, u64) {
    let u = unit(id);
    let Ok((_, out)) = exec(r, &format!("systemctl is-active {u}; ps -o etimes= -p \"$(systemctl show -p MainPID --value {u})\" 2>/dev/null")) else { return (false, 0) };
    let mut lines = out.lines();
    let on = lines.next().map(|l| l.trim() == "active").unwrap_or(false);
    let secs = lines.next().and_then(|l| l.trim().parse().ok()).unwrap_or(0);
    (on, secs)
}

pub fn systemctl(r: &RemoteInfo, id: &str, action: &str) -> Result<(), String> {
    let (code, out) = exec(r, &format!("sudo systemctl {action} {}", unit(id)))?;
    if code != 0 {
        return Err(format!("The server didn't {action} ({}).", out.trim()));
    }
    Ok(())
}

/// The last lines of the server's console log.
pub fn console(r: &RemoteInfo, id: &str, game: &str) -> Vec<String> {
    exec(r, &format!("tail -n 200 \"$HOME/{}/{game}/qconsole.log\" 2>/dev/null", server_dir(id)))
        .map(|(_, o)| o.lines().map(String::from).collect())
        .unwrap_or_default()
}

/// Takes the server off the machine: the service, its folder.
pub fn remove(r: &RemoteInfo, id: &str) -> Result<(), String> {
    let u = unit(id);
    let (code, out) = exec(
        r,
        &format!("sudo systemctl disable --now {u} >/dev/null 2>&1; sudo rm -f /etc/systemd/system/{u}.service; sudo systemctl daemon-reload; rm -rf \"$HOME/{}\"", server_dir(id)),
    )?;
    if code != 0 {
        return Err(format!("Couldn't remove it from the server ({}).", out.trim()));
    }
    Ok(())
}

/// The env assignments for SETUP_SH.
pub fn setup_env(id: &str, game: &str) -> String {
    let mut env = vec![format!("SRVDIR={}", shell_quote(&server_dir(id))), format!("GAME={}", shell_quote(game))];
    env.extend(FOUNDATION_ENV.iter().map(|(k, v)| format!("{k}={}", shell_quote(v))));
    env.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quoting() {
        assert_eq!(shell_quote("a b"), "'a b'");
        assert_eq!(base64_nopad(b"hello"), "aGVsbG8");
        assert_eq!(lf(b"a\r\nb\r\n\rc"), b"a\nb\n\rc");
        assert_eq!(base64_nopad(b"hi!"), "aGkh");
        assert_eq!(shell_quote("it's"), "'it'\\''s'");
        assert!(setup_env("my-server", "czero").contains("GAME='czero'"));
        let svc = service_sh("x", "cstrike", &["-game".into(), "cstrike".into()]);
        assert!(svc.contains("ExecStart=$SRV/hlds_linux '-game' 'cstrike'") && svc.contains("lsctl") && svc.contains("b\"\\xff\\xff\\xff\\xff\""));
    }
}
