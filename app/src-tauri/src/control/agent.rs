//! Shared control for a server on a Linux machine: ls-control (app/server-agent) runs on
//! the server itself, so helpers can help while this computer is off, and nothing needs
//! forwarding on the owner's router. Servers on this computer keep the direct way
//! (control/mod.rs: friends' apps reach this app).
//!
//! On the machine, as the game server's own user:
//! - ~/loner-strike/bin/ls-control, the program (one for every server there), checked
//!   against the sha256 shipped next to it in the app's resources
//! - lonerstrike-<id>-control.service: `ls-control serve` on 127.0.0.1:<port> only
//! - ~/loner-strike/control/<id>/ (only that user can open it): config.json (written here:
//!   the server as helpers see it, never a password), store.json (people and invites,
//!   hashes only), log.jsonl (activity)
//!
//! Helpers reach it through the server's website status front (webstatus.rs): Caddy routes
//! https://<domain>/control/… to it; with a Cloudflare Tunnel the owner adds that path in
//! Cloudflare. Invites carry that https:// address (no certificate fingerprint: Caddy and
//! Cloudflare have real certificates, checked the usual way).
//!
//! This app manages it over the SSH connection it already has, with ls-control's own
//! commands (`ls-control invite …`, which print JSON): no owner key ever goes over HTTP.

use crate::remote::{self, shell_quote, RemoteInfo};
use crate::servers::Entry;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

/// The program on the machine (from the user's home)
pub const BIN: &str = "loner-strike/bin/ls-control";
/// The program in the app's resources (and its sha256 file next to it)
const RES: &str = "linux/ls-control/ls-control";
/// ls-control listens this far above the server's website status port
pub const PORT_OFFSET: u16 = 1000;

pub fn unit(id: &str) -> String {
    format!("lonerstrike-{id}-control")
}

/// Its folder for a server on the machine (from the user's home)
pub fn data_dir(id: &str) -> String {
    format!("loner-strike/control/{id}")
}

/// Whether a server's shared control runs on its own machine (a Linux server) rather than
/// through this app.
pub fn used_for(e: &Entry) -> bool {
    e.remote.is_some()
}

/// The port ls-control listens on (127.0.0.1): the website status service's, plus
/// PORT_OFFSET (one per server on a machine, like those).
pub fn port(e: &Entry) -> u16 {
    let status = e.web_status.port.unwrap_or_else(|| crate::webstatus::port_of(e, &crate::servers::load()));
    status.saturating_add(PORT_OFFSET)
}

/// Where helpers' apps reach it: the website status address, /control. None while the
/// server has no website status page set up.
pub fn url(e: &Entry) -> Option<String> {
    let w = &e.web_status;
    (w.enabled && !w.domain.is_empty()).then(|| format!("https://{}/control", w.domain))
}

/// The program as the app ships it, checked against its sha256 (sha256, bytes).
pub fn shipped(res: &Path) -> Result<(String, Vec<u8>), String> {
    let mut places = vec![res.join(RES)];
    if cfg!(debug_assertions) {
        places.push(crate::setup::repo_root().join("app/src-tauri/resources").join(RES));
    }
    let missing = "This copy of Loner-Strike doesn't include the server's shared control program (ls-control). Reinstalling the app should fix it; in a build from source, run scripts/build-server-agent.sh.";
    let bin = places.into_iter().find(|p| p.is_file()).ok_or(missing)?;
    let bytes = std::fs::read(&bin).map_err(|e| format!("Couldn't read ls-control ({e})."))?;
    let want = std::fs::read_to_string(PathBuf::from(format!("{}.sha256", bin.display()))).map_err(|_| missing.to_string())?;
    let want = want.split_whitespace().next().unwrap_or("").to_ascii_lowercase();
    let have: String = Sha256::digest(&bytes).iter().map(|b| format!("{b:02x}")).collect();
    if want.len() != 64 || want != have || !bytes.starts_with(b"\x7fELF\x02") {
        return Err("The app's copy of ls-control doesn't match its checksum. Reinstalling the app should fix it.".into());
    }
    Ok((have, bytes))
}

/// What ls-control is told about the server: how helpers see it (the same keys the owner's
/// own sharing shows them, never a password), where it is, and its address.
fn config(e: &Entry) -> Value {
    let mut view = super::helper_view(&crate::servers::view(e));
    // (the service works out whether it's running itself)
    if let Some(o) = view.as_object_mut() {
        for k in ["state", "uptime", "error"] {
            o.remove(k);
        }
    }
    json!({"id": e.id, "game": e.game, "port": e.port, "url": url(e).unwrap_or_default(), "front": if e.web_status.mode == "tunnel" { "tunnel" } else { "caddy" }, "view": view})
}

/// A line in the service's unit naming how it's written: a unit without this one is set up
/// again (raise it whenever the unit's text changes)
const UNIT_REV: &str = "loner-strike unit 3";

/// The service's unit, and turning it on (or restarting it with a new program or port).
pub fn install_sh(id: &str, game: &str, port: u16) -> String {
    let u = unit(id);
    let dir = crate::remote::server_dir(id);
    format!(
        r#"set -eu
fail() {{ echo "@@FAIL $*"; exit 3; }}
[ "$(id -u)" = 0 ] && ! command -v sudo >/dev/null && sudo() {{ [ "$1" = -n ] && shift; "$@"; }}
sudo -n true 2>/dev/null || fail "The user $(whoami) can't use sudo without a password, which this needs."
install -d -m 700 "$HOME/loner-strike/control" "$HOME/{data}"
# (the two files it may write: plain files, never links somewhere else)
for f in banned.cfg listip.cfg; do p="$HOME/{dir}/{game}/$f"; [ -L "$p" ] && rm -f "$p"; [ -e "$p" ] || : > "$p"; done
sudo tee /etc/systemd/system/{u}.service >/dev/null <<EOF
[Unit]
# {rev}
Description=Loner-Strike shared control {id}
After=network-online.target
Wants=network-online.target

[Service]
User=$(whoami)
ExecStart=$HOME/{bin} serve --server {id} --port {port}
Restart=always
RestartSec=5
NoNewPrivileges=yes
ProtectSystem=strict
ReadWritePaths=$HOME/{data} $HOME/{dir}/{game}/banned.cfg $HOME/{dir}/{game}/listip.cfg
PrivateTmp=yes
PrivateDevices=yes
RestrictAddressFamilies=AF_INET AF_INET6 AF_UNIX
IPAddressDeny=any
IPAddressAllow=localhost
CapabilityBoundingSet=
AmbientCapabilities=
ProtectKernelTunables=yes
ProtectKernelModules=yes
ProtectKernelLogs=yes
ProtectControlGroups=yes
ProtectClock=yes
RestrictNamespaces=yes
RestrictSUIDSGID=yes
RestrictRealtime=yes
LockPersonality=yes
SystemCallArchitectures=native
SystemCallFilter=@system-service
UMask=077
TasksMax=128
MemoryMax=128M

[Install]
WantedBy=multi-user.target
EOF
sudo systemctl daemon-reload
sudo systemctl enable {u} >/dev/null 2>&1
sudo systemctl restart {u}
sleep 1
systemctl is-active --quiet {u} || fail "Shared control didn't start ($(sudo journalctl -u {u} -n 1 --no-pager -o cat 2>/dev/null))."
echo "@@STEP ready"
"#,
        data = data_dir(id),
        bin = BIN,
        rev = UNIT_REV,
    )
}

/// What takes it off the machine (in remote::remove's command): the service always, its
/// people and log only when the server goes for good (a rebuild keeps who helps run it).
pub fn remove_sh(id: &str, for_good: bool) -> String {
    let u = unit(id);
    let mut sh = format!("sudo systemctl disable --now {u} >/dev/null 2>&1; sudo rm -f /etc/systemd/system/{u}.service; ");
    if for_good {
        sh.push_str(&format!("rm -rf \"$HOME/{}\"; ", data_dir(id)));
    }
    sh
}

/// Puts ls-control on the machine for this server (the program only when it changed),
/// its settings, and its service. Safe to run again.
pub fn install(e: &Entry, res: &Path) -> Result<(), String> {
    let r = e.remote.as_ref().ok_or("Shared control on the server is for servers on a Linux machine.")?;
    let (sha, bytes) = shipped(res)?;
    send_program(r, &sha, &bytes)?;
    write_config(r, e)?;
    let mut last = String::new();
    let code = remote::exec_stream(r, "bash -s", install_sh(&e.id, &e.game, port(e)).as_bytes(), |l| {
        if let Some(why) = l.strip_prefix("@@FAIL ") {
            last = why.to_string();
        } else if !l.starts_with("@@") && !l.trim().is_empty() {
            last = l.trim().to_string();
        }
    })?;
    if code != 0 {
        return Err(format!("Couldn't set up shared control on the server ({last})."));
    }
    Ok(())
}

/// The program, sent beside the old one, checked there, and renamed over it (a running
/// service keeps the copy it started with until it restarts).
fn send_program(r: &RemoteInfo, sha: &str, bytes: &[u8]) -> Result<(), String> {
    let q = shell_quote(BIN);
    let (_, out) = remote::exec(r, &format!("cd \"$HOME\" && mkdir -p loner-strike/bin && if [ -f {q} ]; then sha256sum {q} | cut -c1-64; else echo none; fi"))?;
    if out.trim() == sha {
        return Ok(());
    }
    let new = format!("{BIN}.new");
    remote::write_file(r, &new, bytes)?;
    let n = shell_quote(&new);
    let (code, out) = remote::exec(r, &format!("cd \"$HOME\" && [ \"$(sha256sum {n} | cut -c1-64)\" = {} ] && chmod 755 {n} && mv -f {n} {q}", shell_quote(sha)))?;
    if code != 0 {
        return Err(format!("ls-control didn't arrive whole on the server ({}). Try again.", out.trim()));
    }
    Ok(())
}

fn write_config(r: &RemoteInfo, e: &Entry) -> Result<(), String> {
    let (code, out) = remote::exec(r, &format!("install -d -m 700 \"$HOME/loner-strike/control\" \"$HOME/{}\"", data_dir(&e.id)))?;
    if code != 0 {
        return Err(format!("Couldn't make shared control's folder on the server ({}).", out.trim()));
    }
    let text = serde_json::to_string_pretty(&config(e)).unwrap_or_default();
    // beside it, then renamed over it: the service never reads half a file
    let file = format!("{}/config.json", data_dir(&e.id));
    remote::write_file(r, &format!("{file}.new"), text.as_bytes())?;
    let (code, out) = remote::exec(r, &format!("cd \"$HOME\" && mv -f {} {}", shell_quote(&format!("{file}.new")), shell_quote(&file)))?;
    if code != 0 {
        return Err(format!("Couldn't save shared control's settings on the server ({}).", out.trim()));
    }
    Ok(())
}

/// Makes sure the server's ls-control is this app's version, running on the right port,
/// with the server's settings as they are now; sets it up if not.
pub fn ensure(e: &Entry, res: &Path) -> Result<(), String> {
    let r = e.remote.as_ref().ok_or("Shared control on the server is for servers on a Linux machine.")?;
    // the program itself (two builds of one version differ), running, on the port, and its
    // unit as this app writes it
    let check = format!(
        "sha256sum \"$HOME/{BIN}\" 2>/dev/null | cut -c1-64; echo; systemctl is-active {u} 2>/dev/null; systemctl cat {u} 2>/dev/null | grep -c -- '--port {port}$' || true; systemctl cat {u} 2>/dev/null | grep -c -F '# {UNIT_REV}' || true",
        u = unit(&e.id),
        port = port(e)
    );
    let (sha, _) = shipped(res)?;
    let (_, out) = remote::exec_quick(r, &check)?;
    let lines: Vec<&str> = out.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
    let program_ok = lines.first() == Some(&sha.as_str());
    let running = lines.get(1) == Some(&"active");
    let port_ok = lines.get(2).is_some_and(|n| *n != "0");
    let unit_ok = lines.get(3).is_some_and(|n| *n != "0");
    if program_ok && running && port_ok && unit_ok {
        return write_config(r, e);
    }
    install(e, res)
}

/// One of ls-control's commands for this server, over SSH: its JSON answer, or its error.
pub fn cli(e: &Entry, args: &[&str]) -> Result<Value, String> {
    let r = e.remote.as_ref().ok_or("Shared control on the server is for servers on a Linux machine.")?;
    let mut cmd = format!("\"$HOME/{BIN}\"");
    let server = ["--server", e.id.as_str()];
    for a in args.iter().chain(server.iter()) {
        cmd.push(' ');
        cmd.push_str(&shell_quote(a));
    }
    let (_, out) = remote::exec_quick(r, &cmd)?;
    let v = out.lines().rev().map(str::trim).find(|l| l.starts_with('{')).and_then(|l| serde_json::from_str::<Value>(l).ok()).ok_or_else(|| {
        let said = out.trim().chars().take(200).collect::<String>();
        format!("Shared control on the server didn't answer properly{}.", if said.is_empty() { String::new() } else { format!(" ({said})") })
    })?;
    match v["error"].as_str() {
        Some(e) => Err(e.to_string()),
        None => Ok(v),
    }
}

/// Whether helpers' apps can reach it at its https:// address right now (and its version).
pub fn reachable(url: &str) -> Result<String, String> {
    let r = crate::sys::http().get(format!("{url}/api/control/ping")).timeout(std::time::Duration::from_secs(6)).send().map_err(|e| e.to_string())?;
    let status = r.status().as_u16();
    // the ping is always there: a 404 is the status page answering (a tunnel without the
    // /control/ route, or with it below the status route), a 5xx the front not reaching it
    match status {
        404 => return Err("the status page answered instead. In Cloudflare, add the ^/control/ route and move it above the status route".into()),
        502..=504 | 530 => return Err(format!("the front couldn't reach shared control on the server (HTTP {status})")),
        _ => {}
    }
    let v: Value = r.json().map_err(|_| format!("it answered with something else (HTTP {status})"))?;
    v["version"].as_str().map(String::from).ok_or_else(|| format!("it answered with something else (HTTP {status})"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn service_text() {
        let sh = install_sh("friday", "cstrike", 9090);
        assert!(sh.contains("ExecStart=$HOME/loner-strike/bin/ls-control serve --server friday --port 9090"));
        assert!(sh.contains("ReadWritePaths=$HOME/loner-strike/control/friday $HOME/loner-strike/servers/friday/cstrike/banned.cfg $HOME/loner-strike/servers/friday/cstrike/listip.cfg"));
        assert!(sh.contains("NoNewPrivileges=yes") && sh.contains("User=$(whoami)") && !sh.contains("0.0.0.0"));
        assert!(remove_sh("friday", true).contains("rm -rf \"$HOME/loner-strike/control/friday\""));
        assert!(!remove_sh("friday", false).contains("rm -rf"), "a rebuild keeps who helps run it");
    }

    #[test]
    #[cfg(unix)]
    fn scripts_parse() {
        use std::io::Write;
        for sh in [install_sh("friday", "czero", 9091), format!("{}true", remove_sh("friday", true))] {
            let mut p = std::process::Command::new("bash").arg("-n").stdin(std::process::Stdio::piped()).spawn().unwrap();
            p.stdin.take().unwrap().write_all(sh.as_bytes()).unwrap();
            assert!(p.wait().unwrap().success(), "{sh}");
        }
    }
}
