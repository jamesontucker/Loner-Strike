//! Website status for a server on a Linux machine (a VPS): a tiny service on the machine
//! that tells a website the server's live name, map and players, and can count visits.
//!
//! On the machine: webstatus.py (stdlib Python) at /opt/lonerstrike-status/status.py,
//! root-owned, run by `lonerstrike-<id>-status` as its own system user
//! (lonerstrike-status: no login, no home, no sudo) on 127.0.0.1:<port>, with its visit
//! count in /var/lib/lonerstrike-status/<id>. It asks the game on the same machine with
//! A2S_INFO over UDP.
//!
//! HTTPS in front of it, one of two ways:
//! - "caddy": Caddy from the distro (or Caddy's own apt repo), one site block per server in
//!   /etc/caddy/loner-strike.d/<id>.caddy, TCP 80 and 443 opened in the machine's firewall.
//!   Caddy gets the certificate itself once the DNS record points at the machine.
//! - "tunnel": a Cloudflare Tunnel the host made in Cloudflare's dashboard; cloudflared
//!   (Cloudflare's apt repo) runs as `lonerstrike-<id>-tunnel` with the tunnel's token in a
//!   root-only file. Nothing is opened in the firewall.

use crate::panel_api::from_panel;
use crate::server::Shared;
use crate::servers::{self, Entry};
use axum::extract::Path as UrlPath;
use axum::http::{HeaderMap, StatusCode};
use axum::routing::get as route_get;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::net::{IpAddr, ToSocketAddrs};
use std::path::PathBuf;
use std::time::Duration;

/// The service itself, written to the machine as /opt/lonerstrike-status/status.py
pub const STATUS_PY: &str = include_str!("webstatus.py");

/// The service's own system user on the machine
pub const USER: &str = "lonerstrike-status";
const SCRIPT: &str = "/opt/lonerstrike-status/status.py";
const DATA: &str = "/var/lib/lonerstrike-status";
/// Root-only folder for tunnel tokens on the machine
const SECRETS: &str = "/etc/lonerstrike-status";
const CADDY_DIR: &str = "/etc/caddy/loner-strike.d";
const CADDY_IMPORT: &str = "import /etc/caddy/loner-strike.d/*.caddy";
/// Status services take ports from here up, one per server on a machine
const FIRST_PORT: u16 = 8090;
const LAST_PORT: u16 = 8189;
const MAX_ORIGINS: usize = 10;

// ---- settings --------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Settings {
    pub enabled: bool,
    /// The status page's host name, like status.example.com
    pub domain: String,
    /// Sites allowed to read the answers (https://example.com)
    pub origins: Vec<String>,
    /// Counts visits at /visit
    pub counter: bool,
    /// "caddy" (its own HTTPS, TCP 80 and 443) or "tunnel" (a Cloudflare Tunnel)
    pub mode: String,
    /// The port on 127.0.0.1 the service listens on, picked once and kept (a tunnel's
    /// hostname in Cloudflare points at it)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
    /// Tunnel mode with the app's own cloudflared service (from a token pasted in the app);
    /// false: a Cloudflare Tunnel already on the machine (the stock cloudflared.service,
    /// set up by hand), which the app uses but never changes or removes
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub own_tunnel: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { enabled: false, domain: String::new(), origins: Vec::new(), counter: false, mode: "caddy".into(), port: None, own_tunnel: false }
    }
}

impl Settings {
    pub fn is_unset(&self) -> bool {
        *self == Settings::default()
    }
    fn tunnel(&self) -> bool {
        self.mode == "tunnel"
    }
    /// The app runs its own cloudflared for this server
    fn own_tunnel(&self) -> bool {
        self.tunnel() && self.own_tunnel
    }
}

pub fn unit(id: &str) -> String {
    format!("lonerstrike-{id}-status")
}

pub fn tunnel_unit(id: &str) -> String {
    format!("lonerstrike-{id}-tunnel")
}

const DOMAIN_HELP: &str = "Enter a name like status.example.com.";

/// A host name for the status page: at least two labels, letters, digits and dashes,
/// not an IP address. Comes back lowercase.
pub fn clean_domain(text: &str) -> Result<String, String> {
    let t = text.trim().trim_end_matches('.').to_ascii_lowercase();
    let t = t.strip_prefix("https://").or_else(|| t.strip_prefix("http://")).unwrap_or(&t).trim_end_matches('/').to_string();
    if t.is_empty() || t.len() > 253 || !t.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-') {
        return Err(DOMAIN_HELP.into());
    }
    let labels: Vec<&str> = t.split('.').collect();
    let label_ok = |l: &&str| !l.is_empty() && l.len() <= 63 && !l.starts_with('-') && !l.ends_with('-');
    if labels.len() < 2 || !labels.iter().all(label_ok) || labels.last().unwrap().chars().all(|c| c.is_ascii_digit()) {
        return Err(DOMAIN_HELP.into());
    }
    Ok(t)
}

/// A site allowed to read the answers: https://host[:port], https://*.suffix (any subdomain
/// of it, like Cloudflare Pages' previews), or http://localhost[:port] for trying a site on
/// your own computer. No path. Comes back lowercase, without a final /.
pub fn clean_origin(text: &str) -> Result<String, String> {
    let bad = || format!("\"{}\" isn't a site address. Enter one like https://example.com.", text.trim());
    let t = text.trim().trim_end_matches('/').to_ascii_lowercase();
    let (scheme, rest) = t.split_once("://").ok_or_else(bad)?;
    let (host, port) = match rest.rsplit_once(':') {
        Some((h, p)) => {
            if p.is_empty() || !p.chars().all(|c| c.is_ascii_digit()) || p.parse::<u16>().map_or(true, |n| n == 0) {
                return Err(bad());
            }
            (h, Some(p))
        }
        None => (rest, None),
    };
    let local = host == "localhost" || host == "127.0.0.1";
    if let Some(suffix) = host.strip_prefix("*.") {
        if scheme != "https" {
            return Err(format!("Use https:// for {host}."));
        }
        clean_domain(suffix).map_err(|_| bad())?;
        return Ok(match port {
            Some(p) => format!("https://{host}:{p}"),
            None => format!("https://{host}"),
        });
    }
    match scheme {
        "https" => {}
        "http" if local => {}
        "http" => return Err(format!("Use https:// for {host}. Only a site on your own computer (localhost) can use http://.")),
        _ => return Err(bad()),
    }
    if !local {
        clean_domain(host).map_err(|_| bad())?;
    }
    Ok(match port {
        Some(p) => format!("{scheme}://{host}:{p}"),
        None => format!("{scheme}://{host}"),
    })
}

/// A Cloudflare Tunnel token as pasted: the token alone, or the whole
/// `cloudflared service install <token>` line Cloudflare shows (the token is its last word).
pub fn clean_token(text: &str) -> Result<String, String> {
    let t = text.split_whitespace().last().unwrap_or("");
    let ok = (40..=4096).contains(&t.len()) && t.chars().all(|c| c.is_ascii_alphanumeric() || "+/=_-".contains(c));
    if !ok {
        return Err("That doesn't look like a tunnel token. Copy it from the tunnel's Install connector page in Cloudflare: the long text after cloudflared service install.".into());
    }
    Ok(t.to_string())
}

/// Another server on the same machine.
fn same_machine(me: &Entry, o: &Entry) -> bool {
    o.id != me.id && o.remote.as_ref().zip(me.remote.as_ref()).is_some_and(|(a, b)| a.host == b.host)
}

/// The port the service has, or the one it would get.
pub fn port_of(me: &Entry, all: &[Entry]) -> u16 {
    let taken: Vec<u16> = all.iter().filter(|o| same_machine(me, o)).filter_map(|o| o.web_status.port).collect();
    match me.web_status.port {
        Some(p) if !taken.contains(&p) => p,
        _ => (FIRST_PORT..=LAST_PORT).find(|p| !taken.contains(p)).unwrap_or(LAST_PORT),
    }
}

/// Checks and tidies new settings for server `me` (`all`: every server).
pub fn validate(mut s: Settings, me: &Entry, all: &[Entry]) -> Result<Settings, String> {
    if me.remote.is_none() {
        return Err("Website status is for servers on a Linux machine.".into());
    }
    if s.mode != "tunnel" {
        s.mode = "caddy".into();
    }
    s.domain = if s.domain.trim().is_empty() { String::new() } else { clean_domain(&s.domain)? };
    let mut origins: Vec<String> = Vec::new();
    for o in s.origins.iter().filter(|o| !o.trim().is_empty()) {
        let o = clean_origin(o)?;
        if !origins.contains(&o) {
            origins.push(o);
        }
    }
    if origins.len() > MAX_ORIGINS {
        return Err(format!("Enter at most {MAX_ORIGINS} sites."));
    }
    s.origins = origins;
    if s.enabled {
        if s.domain.is_empty() {
            return Err("Enter the status page's address, like status.example.com.".into());
        }
        if s.origins.is_empty() {
            return Err("Enter at least one site that may show the status, like https://example.com.".into());
        }
        if let Some(o) = all.iter().find(|o| o.id != me.id && o.web_status.enabled && o.web_status.domain == s.domain) {
            return Err(format!("\"{}\" already uses {}. Pick another address.", o.name, s.domain));
        }
    }
    // picked once and kept, so a tunnel's hostname in Cloudflare stays right
    let probe = Entry { web_status: s.clone(), ..me.clone() };
    s.port = Some(port_of(&probe, all));
    Ok(s)
}

/// A name for the status page next to the server's public address: status.<its domain>.
pub fn suggested_domain(public_address: Option<&str>) -> Option<String> {
    let host = public_address?.split(':').next()?.to_ascii_lowercase();
    if host.parse::<std::net::Ipv4Addr>().is_ok() {
        return None;
    }
    let labels: Vec<&str> = host.split('.').filter(|l| !l.is_empty()).collect();
    match labels.len() {
        0 | 1 => None,
        2 => Some(format!("status.{}", labels.join("."))),
        _ => Some(format!("status.{}", labels[1..].join("."))),
    }
}

// ---- the tunnel token (kept on this computer, like the app's other secrets) -------------

fn token_path(id: &str) -> PathBuf {
    crate::sys::app_dir().join("secrets").join(format!("tunnel-{id}.token"))
}

fn save_token(id: &str, token: &str) -> Result<(), String> {
    let p = token_path(id);
    if let Some(d) = p.parent() {
        std::fs::create_dir_all(d).map_err(|e| format!("Couldn't save the token ({e})."))?;
    }
    std::fs::write(&p, token).map_err(|e| format!("Couldn't save the token ({e})."))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

fn read_token(id: &str) -> Option<String> {
    std::fs::read_to_string(token_path(id)).ok().map(|t| t.trim().to_string()).filter(|t| !t.is_empty())
}

/// Forgets a server's tunnel token (the server was deleted).
pub fn forget_token(id: &str) {
    let _ = std::fs::remove_file(token_path(id));
}

// ---- what runs on the machine ----------------------------------------------------------

/// The status service's systemd unit.
pub fn status_unit_text(id: &str, listen: u16, game_port: u16, origins: &[String], counter: bool) -> String {
    let counter = if counter { format!(" --counter {DATA}/{id}/visits.txt") } else { String::new() };
    format!(
        "[Unit]
Description=Loner-Strike website status {id}
After=network-online.target
Wants=network-online.target

[Service]
User={USER}
Group={USER}
ExecStart=/usr/bin/python3 {SCRIPT} --listen {listen} --game {game_port} --origins {origins}{counter}
Restart=always
RestartSec=5
NoNewPrivileges=yes
ProtectSystem=strict
ProtectHome=yes
PrivateTmp=yes
PrivateDevices=yes
ReadWritePaths={DATA}/{id}
RestrictAddressFamilies=AF_INET AF_INET6
CapabilityBoundingSet=
AmbientCapabilities=
MemoryMax=64M

[Install]
WantedBy=multi-user.target
",
        origins = origins.join(",")
    )
}

/// cloudflared for one server's tunnel, as a throwaway user. `token_file`: this cloudflared
/// takes --token-file (the token comes in through systemd's credentials, from a root-only
/// file); otherwise the token is read from a root-only environment file. Either way it never
/// shows in the process list.
pub fn tunnel_unit_text(id: &str, token_file: bool) -> String {
    let (load, exec) = if token_file {
        (format!("LoadCredential=token:{SECRETS}/tunnel-{id}.token"), "/usr/bin/cloudflared tunnel --no-autoupdate run --token-file %d/token".to_string())
    } else {
        (format!("EnvironmentFile={SECRETS}/tunnel-{id}.env"), "/usr/bin/cloudflared tunnel --no-autoupdate run".to_string())
    };
    format!(
        "[Unit]
Description=Loner-Strike website status tunnel {id}
After=network-online.target
Wants=network-online.target

[Service]
DynamicUser=yes
{load}
ExecStart={exec}
Restart=always
RestartSec=5
NoNewPrivileges=yes
ProtectSystem=strict
ProtectHome=yes
PrivateTmp=yes
PrivateDevices=yes
RestrictAddressFamilies=AF_INET AF_INET6 AF_UNIX AF_NETLINK
CapabilityBoundingSet=
AmbientCapabilities=
MemoryMax=256M

[Install]
WantedBy=multi-user.target
"
    )
}

/// Caddy's site block: only /status and /visit go to the service, everything else is a 404.
pub fn caddy_site(domain: &str, port: u16) -> String {
    format!(
        "# Loner-Strike website status. Written by the app; changes here are replaced.
{domain} {{
\theader -Server
\t@status path /status /visit
\thandle @status {{
\t\treverse_proxy 127.0.0.1:{port} {{
\t\t\theader_down -Server
\t\t}}
\t}}
\thandle {{
\t\trespond 404
\t}}
}}
"
    )
}

/// The start of every script: sudo (or root without it), apt, and failing with a reason.
const HEAD: &str = r#"set -eu
fail() { echo "@@FAIL $*"; exit 3; }
[ "$(id -u)" = 0 ] && ! command -v sudo >/dev/null && sudo() { [ "$1" = -n ] && shift; "$@"; }
sudo -n true 2>/dev/null || fail "The user $(whoami) can't use sudo without a password, which this needs."
apt_get() { sudo env DEBIAN_FRONTEND=noninteractive NEEDRESTART_MODE=a apt-get -o DPkg::Lock::Timeout=600 "$@" </dev/null; }
"#;

/// Takes away the Caddy site block (Caddy itself stays installed).
const DROP_CADDY: &str = r#"if [ -f @CADDY_DIR@/@ID@.caddy ]; then
  sudo rm -f @CADDY_DIR@/@ID@.caddy
  if systemctl is-active --quiet caddy; then sudo systemctl reload caddy >/dev/null 2>&1 || true; fi
fi
"#;

/// Takes away the tunnel (cloudflared itself stays installed).
const DROP_TUNNEL: &str = r#"sudo systemctl disable --now @TUNIT@ >/dev/null 2>&1 || true
sudo rm -f /etc/systemd/system/@TUNIT@.service @SECRETS@/tunnel-@ID@.token @SECRETS@/tunnel-@ID@.env
"#;

const INSTALL_SERVICE: &str = r#"echo "@@STEP service"
[ -x /usr/bin/python3 ] || fail "Python 3 isn't on the server. Press Update in Overview, under Maintenance, then try again."
# its own user: no login, no home, no sudo
id -u @USER@ >/dev/null 2>&1 || sudo useradd --system --no-create-home --home-dir /nonexistent --shell /usr/sbin/nologin @USER@
sudo install -d -o root -g root -m 755 /opt/lonerstrike-status @DATA@
sudo install -d -o @USER@ -g @USER@ -m 750 @DATA@/@ID@
sudo tee @SCRIPT@ >/dev/null <<'LSPY'
@PY@LSPY
sudo chown root:root @SCRIPT@
sudo chmod 644 @SCRIPT@
sudo tee /etc/systemd/system/@UNIT@.service >/dev/null <<'LSUNIT'
@UNIT_TEXT@LSUNIT
"#;

const INSTALL_CADDY: &str = r##"echo "@@STEP caddy"
if ! command -v caddy >/dev/null; then
  for i in 1 2 3; do apt_get update -qq >/dev/null 2>&1 && break; sleep 10; done
  if ! apt-cache policy caddy 2>/dev/null | grep -q 'Candidate: [0-9]'; then
    # not in this release's own packages: Caddy's official repository
    apt_get install -y -qq curl gnupg ca-certificates >/dev/null
    curl -fsSL --proto '=https' https://dl.cloudsmith.io/public/caddy/stable/gpg.key | sudo gpg --dearmor --yes -o /usr/share/keyrings/caddy-stable-archive-keyring.gpg
    curl -fsSL --proto '=https' https://dl.cloudsmith.io/public/caddy/stable/debian.deb.txt | sudo tee /etc/apt/sources.list.d/caddy-stable.list >/dev/null
    apt_get update -qq >/dev/null 2>&1 || true
  fi
  apt_get install -y -qq caddy >/dev/null || fail "Couldn't install Caddy, the web server that gives the status page HTTPS."
fi
other=$(sudo ss -Hltnp 2>/dev/null | awk '$4 ~ /:(80|443)$/' | grep -v '"caddy"' || true)
[ -z "$other" ] || fail "Another program already uses port 80 or 443 on the server (a web server?). Stop it, or pick Cloudflare Tunnel."

echo "@@STEP firewall"
if command -v iptables >/dev/null; then
  for p in 80 443; do
    if ! sudo iptables -C INPUT -p tcp --dport $p -j ACCEPT 2>/dev/null; then
      # above a final REJECT rule if there is one (Oracle's images have one)
      pos=$(sudo iptables -L INPUT --line-numbers -n | awk '$2=="REJECT" {print $1; exit}')
      if [ -n "$pos" ]; then sudo iptables -I INPUT "$pos" -p tcp --dport $p -j ACCEPT
      else sudo iptables -A INPUT -p tcp --dport $p -j ACCEPT; fi
    fi
  done
  if command -v netfilter-persistent >/dev/null; then sudo netfilter-persistent save >/dev/null 2>&1 || true
  elif [ -d /etc/iptables ]; then sudo sh -c 'iptables-save > /etc/iptables/rules.v4'; fi
fi
if command -v ufw >/dev/null && sudo ufw status | grep -q "Status: active"; then sudo ufw allow 80/tcp >/dev/null; sudo ufw allow 443/tcp >/dev/null; fi

echo "@@STEP site"
sudo mkdir -p @CADDY_DIR@
sudo tee @CADDY_DIR@/@ID@.caddy >/dev/null <<'LSCADDY'
@CADDY_SITE@LSCADDY
CF=/etc/caddy/Caddyfile
# Caddy's own example page (the file as installed) is replaced; anything else is kept
stock=$(grep -v '^[[:space:]]*#' "$CF" 2>/dev/null | tr -d ' \t' | grep -v '^$' | tr '\n' '|' || true)
if [ ! -s "$CF" ] || [ "$stock" = ':80{|root*/usr/share/caddy|file_server|}|' ]; then
  printf '%s\n' "# Written by Loner-Strike: each status page has a file in @CADDY_DIR@" "@IMPORT@" | sudo tee "$CF" >/dev/null
elif ! sudo grep -qxF "@IMPORT@" "$CF"; then
  printf '\n%s\n' "@IMPORT@" | sudo tee -a "$CF" >/dev/null
fi
if ! sudo caddy adapt --config "$CF" --adapter caddyfile >/dev/null 2>&1; then
  sudo rm -f @CADDY_DIR@/@ID@.caddy
  fail "Caddy didn't accept the settings. Is @DOMAIN@ already used in /etc/caddy/Caddyfile?"
fi
"##;

const INSTALL_TUNNEL: &str = r#"echo "@@STEP cloudflared"
if ! command -v cloudflared >/dev/null; then
  apt_get install -y -qq curl ca-certificates >/dev/null
  sudo install -d -m 755 /usr/share/keyrings
  curl -fsSL --proto '=https' https://pkg.cloudflare.com/cloudflare-main.gpg | sudo tee /usr/share/keyrings/cloudflare-main.gpg >/dev/null
  echo 'deb [signed-by=/usr/share/keyrings/cloudflare-main.gpg] https://pkg.cloudflare.com/cloudflared any main' | sudo tee /etc/apt/sources.list.d/cloudflared.list >/dev/null
  for i in 1 2 3; do apt_get update -qq >/dev/null 2>&1 && break; sleep 10; done
  apt_get install -y -qq cloudflared >/dev/null || fail "Couldn't install cloudflared, Cloudflare's tunnel program."
fi
[ -x /usr/bin/cloudflared ] || fail "cloudflared isn't where it should be (/usr/bin/cloudflared)."
# the token: root-only, sent through this script's input (never on a command line)
sudo install -d -o root -g root -m 700 @SECRETS@
sudo sh -c 'umask 077; cat > @SECRETS@/tunnel-@ID@.token' <<'LSTOKEN'
@TOKEN@
LSTOKEN
if /usr/bin/cloudflared tunnel run --help 2>&1 | grep -q -- '--token-file'; then
  sudo rm -f @SECRETS@/tunnel-@ID@.env
  sudo tee /etc/systemd/system/@TUNIT@.service >/dev/null <<'LSUNIT'
@TUNNEL_FILE@LSUNIT
else
  sudo sh -c 'umask 077; printf "TUNNEL_TOKEN=%s\n" "$(cat @SECRETS@/tunnel-@ID@.token)" > @SECRETS@/tunnel-@ID@.env'
  sudo tee /etc/systemd/system/@TUNIT@.service >/dev/null <<'LSUNIT'
@TUNNEL_ENV@LSUNIT
fi
"#;

const START: &str = r#"echo "@@STEP start"
sudo systemctl daemon-reload
sudo systemctl enable @UNIT@ >/dev/null 2>&1
sudo systemctl restart @UNIT@
@FRONT@
sleep 3
systemctl is-active --quiet @UNIT@ || fail "The status service didn't start ($(sudo journalctl -u @UNIT@ -n 1 --no-pager -o cat 2>/dev/null))."
@CHECK_FRONT@
echo "@@STEP ready"
"#;

/// Tunnel mode with the machine's own Cloudflare Tunnel: it has to be running.
const USE_EXISTING: &str = r#"systemctl is-active --quiet cloudflared || fail "The Cloudflare Tunnel on this machine (cloudflared) isn't running. Start it with sudo systemctl start cloudflared, or paste a tunnel token in the app.""#;

/// Is there a Cloudflare Tunnel on the machine the host set up by hand (the stock
/// cloudflared.service)? "running", "stopped", or None when there's none.
fn existing_tunnel(r: &crate::remote::RemoteInfo) -> Result<Option<&'static str>, String> {
    let (_, out) = crate::remote::exec_quick(r, "if systemctl is-active --quiet cloudflared; then echo running; elif [ -f /etc/systemd/system/cloudflared.service ] || [ -f /lib/systemd/system/cloudflared.service ]; then echo stopped; else echo none; fi")?;
    Ok(match out.trim() {
        "running" => Some("running"),
        "stopped" => Some("stopped"),
        _ => None,
    })
}

fn fill(t: &str, id: &str) -> String {
    t.replace("@CADDY_DIR@", CADDY_DIR)
        .replace("@IMPORT@", CADDY_IMPORT)
        .replace("@SECRETS@", SECRETS)
        .replace("@SCRIPT@", SCRIPT)
        .replace("@DATA@", DATA)
        .replace("@USER@", USER)
        .replace("@TUNIT@", &tunnel_unit(id))
        .replace("@UNIT@", &unit(id))
        .replace("@ID@", id)
}

/// Sets up (or updates) the status service and its HTTPS front on the machine, taking away
/// the other front if it was used before. `token` is needed for a tunnel. Safe to run again.
/// Progress lines start with @@STEP; a failure with @@FAIL.
pub fn install_sh(id: &str, s: &Settings, listen: u16, game_port: u16, token: Option<&str>) -> String {
    let mut sh = String::from(HEAD);
    // (the script and unit go in as they are: substitutions happen before, in the template)
    let service = fill(INSTALL_SERVICE, id);
    let unit_text = status_unit_text(id, listen, game_port, &s.origins, s.counter);
    sh.push_str(&service.replacen("@PY@", STATUS_PY, 1).replacen("@UNIT_TEXT@", &unit_text, 1));
    if s.tunnel() && token.is_none() {
        // the Cloudflare Tunnel already on the machine (cloudflared.service): used, never changed
        sh.push_str(&fill(DROP_CADDY, id));
        sh.push_str(&fill(DROP_TUNNEL, id));
        sh.push_str(&fill(START, id).replace("@FRONT@", "").replace("@CHECK_FRONT@", USE_EXISTING));
    } else if s.tunnel() {
        sh.push_str(&fill(DROP_CADDY, id));
        let t = fill(INSTALL_TUNNEL, id);
        sh.push_str(
            &t.replacen("@TUNNEL_FILE@", &tunnel_unit_text(id, true), 1)
                .replacen("@TUNNEL_ENV@", &tunnel_unit_text(id, false), 1)
                .replacen("@TOKEN@", token.unwrap_or(""), 1),
        );
        let tu = tunnel_unit(id);
        sh.push_str(
            &fill(START, id)
                .replace("@FRONT@", &format!("sudo systemctl enable {tu} >/dev/null 2>&1\nsudo systemctl restart {tu}"))
                .replace(
                    "@CHECK_FRONT@",
                    &format!("systemctl is-active --quiet {tu} || fail \"The tunnel didn't start ($(sudo journalctl -u {tu} -n 1 --no-pager -o cat 2>/dev/null)). Check the token.\""),
                ),
        );
    } else {
        sh.push_str(&fill(DROP_TUNNEL, id));
        sh.push_str(&fill(INSTALL_CADDY, id).replace("@DOMAIN@", &s.domain).replacen("@CADDY_SITE@", &caddy_site(&s.domain, listen), 1));
        sh.push_str(
            &fill(START, id)
                .replace("@FRONT@", "sudo systemctl enable caddy >/dev/null 2>&1 || true\nsudo systemctl reload-or-restart caddy || fail \"Caddy didn't start. Something else may be using port 80 or 443.\"")
                .replace("@CHECK_FRONT@", "systemctl is-active --quiet caddy || fail \"Caddy didn't start. Something else may be using port 80 or 443.\""),
        );
    }
    sh
}

/// Turns the status page off: the service and the tunnel stopped and their units gone,
/// Caddy's site block gone (Caddy stays). The visit count is kept for next time.
pub fn off_sh(id: &str) -> String {
    let mut sh = String::from("[ \"$(id -u)\" = 0 ] && ! command -v sudo >/dev/null && sudo() { [ \"$1\" = -n ] && shift; \"$@\"; }\n");
    sh.push_str(&format!("sudo systemctl disable --now {} >/dev/null 2>&1 || true\n", unit(id)));
    sh.push_str(&format!("sudo rm -f /etc/systemd/system/{}.service\n", unit(id)));
    sh.push_str(&fill(DROP_TUNNEL, id));
    sh.push_str(&fill(DROP_CADDY, id));
    sh.push_str("sudo systemctl daemon-reload\n");
    sh
}

/// For removing a server: everything off_sh does, and its visit count too.
pub fn remove_sh(id: &str) -> String {
    format!("{}sudo rm -rf {DATA}/{id}\n", off_sh(id))
}

// ---- applying --------------------------------------------------------------------------

/// Writes the status service on the machine and turns it on, or takes it away.
pub fn apply(e: &Entry, on: bool) -> Result<(), String> {
    let r = e.remote.as_ref().ok_or("Website status is for servers on a Linux machine.")?;
    let script = if on {
        let s = &e.web_status;
        let listen = s.port.unwrap_or(FIRST_PORT);
        let token = if s.own_tunnel() { Some(read_token(&e.id).ok_or("Paste the tunnel's token first.")?) } else { None };
        install_sh(&e.id, s, listen, e.port, token.as_deref())
    } else {
        off_sh(&e.id)
    };
    let mut failed = String::new();
    let mut last = String::new();
    let code = crate::remote::exec_stream(r, "bash -s", script.as_bytes(), |line| {
        if let Some(why) = line.strip_prefix("@@FAIL ") {
            failed = why.to_string();
        } else if !line.starts_with("@@") && !line.trim().is_empty() {
            last = line.trim().chars().take(160).collect();
        }
    })?;
    match code {
        0 => Ok(()),
        _ if !failed.is_empty() => Err(failed),
        _ if on => Err(format!("Couldn't set up the status page on the server{}.", if last.is_empty() { String::new() } else { format!(" ({last})") })),
        _ => Err("Couldn't turn the status page off on the server.".into()),
    }
}

/// After a rebuild or a port change: written again if it's on (errors show in its status).
pub fn refresh(id: &str) {
    if let Some(e) = servers::get(id).filter(|e| e.remote.is_some() && e.web_status.enabled) {
        let _ = apply(&e, true);
    }
}

// ---- is it answering -------------------------------------------------------------------

fn error_text(e: &dyn std::error::Error) -> String {
    let mut s = e.to_string();
    let mut src = e.source();
    while let Some(x) = src {
        s.push_str(": ");
        s.push_str(&x.to_string());
        src = x.source();
    }
    s.to_lowercase()
}

fn resolve(host: &str) -> Vec<IpAddr> {
    (host, 443).to_socket_addrs().map(|a| a.map(|x| x.ip()).collect()).unwrap_or_default()
}

/// What the status page answers from the internet: Ok(the answer) or Err(why not, in plain words).
fn check(e: &Entry) -> Result<Value, String> {
    let s = &e.web_status;
    let d = &s.domain;
    let host = e.remote.as_ref().map(|r| r.host.clone()).unwrap_or_default();
    let ips = resolve(d);
    if ips.is_empty() {
        return Err(if s.tunnel() {
            format!("The name {d} doesn't point anywhere yet. Add it as a Public hostname on the tunnel in Cloudflare.")
        } else {
            format!("The name {d} doesn't point anywhere yet. Add its DNS record. New records can take a few minutes to show up.")
        });
    }
    let res = crate::sys::http().get(format!("https://{d}/status")).timeout(Duration::from_secs(6)).send();
    match res {
        Ok(resp) => {
            let code = resp.status().as_u16();
            let text = resp.text().unwrap_or_default();
            let v: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
            if code == 200 && v.get("online").is_some() {
                return Ok(v);
            }
            Err(match code {
                502 | 503 | 504 if !s.tunnel() => format!("Caddy answers, but the status service behind it doesn't (error {code})."),
                502 | 503 | 504 => format!("Cloudflare reaches the tunnel, but not the status service (error {code}). Check the hostname points at http://localhost:{}.", s.port.unwrap_or(FIRST_PORT)),
                530 | 1033 => "Cloudflare can't reach the tunnel. Check the tunnel shows Healthy in Cloudflare and the token is right.".into(),
                521..=526 => format!("Cloudflare couldn't reach the server (error {code}). Set Cloudflare's SSL/TLS mode to Full, or turn the orange cloud off (DNS only), and check TCP 80 and 443 are open."),
                404 => format!("{d} answers, but not with the status page (error 404). Check the DNS record points at this server ({host})."),
                _ => format!("{d} answered with error {code}."),
            })
        }
        Err(err) => {
            let t = error_text(&err);
            let at_server = resolve(&host);
            let elsewhere = !s.tunnel() && !at_server.is_empty() && !ips.iter().any(|ip| at_server.contains(ip));
            let points = || format!("{d} points at {}, not at this server ({host}). Fix the DNS record, or wait a few minutes after changing it.", ips.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(", "));
            Err(if t.contains("certificate") || t.contains("tls") || t.contains("handshake") || t.contains("ssl") {
                if elsewhere {
                    points()
                } else {
                    "The HTTPS certificate isn't ready yet. Caddy gets one by itself once the DNS record points at this server and TCP 80 and 443 are open. That can take a minute.".into()
                }
            } else if elsewhere {
                points()
            } else if err.is_timeout() {
                format!("No answer from {d}. Check your provider's firewall allows TCP 80 and 443 to the server.")
            } else if t.contains("refused") {
                format!("{d} refused the connection. Check Caddy is running and port 443 is open.")
            } else {
                format!("Couldn't reach {d} ({}).", err.without_url())
            })
        }
    }
}

fn status(e: &Entry) -> Value {
    let all = servers::load();
    let s = &e.web_status;
    let port = port_of(e, &all);
    let (state, reason, live) = if !s.enabled || s.domain.is_empty() {
        ("off", None, None)
    } else {
        match check(e) {
            Ok(v) => ("running", None, Some(v)),
            Err(why) => {
                // the machine itself may say more: a service that isn't running
                let mut why = why;
                if let Some(r) = &e.remote {
                    let u = if s.own_tunnel() { tunnel_unit(&e.id) } else if s.tunnel() { "cloudflared".into() } else { "caddy".into() };
                    for (unit_name, what) in [(unit(&e.id), "The status service"), (u, if s.tunnel() { "The tunnel" } else { "Caddy" })] {
                        if let Ok((false, _)) = crate::remote::unit_state(r, &unit_name) {
                            why = format!("{what} isn't running on the server. Press Install again. {why}");
                            break;
                        }
                    }
                }
                ("not_answering", Some(why), None)
            }
        }
    };
    // only asked about while tunnel mode is picked or could be (a remote server)
    let existing = e.remote.as_ref().and_then(|r| existing_tunnel(r).ok().flatten());
    let urls = (!s.domain.is_empty()).then(|| (format!("https://{}/status", s.domain), format!("https://{}/visit", s.domain)));
    json!({
        "remote": e.remote.is_some(),
        "host": e.remote.as_ref().map(|r| r.host.clone()),
        "settings": {"enabled": s.enabled, "domain": s.domain, "origins": s.origins, "counter": s.counter, "mode": s.mode},
        "token_saved": read_token(&e.id).is_some(),
        "own_tunnel": s.own_tunnel(),
        "existing_tunnel": existing,
        "port": port,
        "suggested_domain": suggested_domain(e.public_address.as_deref()),
        "state": state,
        "reason": reason,
        "live": live,
        "status_url": urls.as_ref().map(|u| u.0.clone()),
        "visit_url": urls.as_ref().filter(|_| s.counter).map(|u| u.1.clone()),
    })
}

// ---- API -------------------------------------------------------------------------------

fn answer(r: Result<Value, String>) -> (StatusCode, Json<Value>) {
    match r {
        Ok(v) => (StatusCode::OK, Json(v)),
        Err(e) => (StatusCode::OK, Json(json!({"error": e}))),
    }
}

async fn blocking(f: impl FnOnce() -> Result<Value, String> + Send + 'static) -> (StatusCode, Json<Value>) {
    answer(tokio::task::spawn_blocking(f).await.unwrap_or_else(|_| Err("Loner-Strike hit an internal error. Try again, and copy the diagnostics from Settings if it keeps happening.".into())))
}

fn entry(id: &str) -> Result<Entry, String> {
    servers::get(id).ok_or_else(|| "That server isn't there any more.".to_string())
}

async fn api_status(UrlPath(id): UrlPath<String>) -> (StatusCode, Json<Value>) {
    blocking(move || entry(&id).map(|e| status(&e))).await
}

#[derive(Deserialize)]
struct SaveReq {
    enabled: bool,
    #[serde(default)]
    domain: String,
    #[serde(default)]
    origins: Vec<String>,
    #[serde(default)]
    counter: bool,
    #[serde(default)]
    mode: String,
    /// a new tunnel token; empty keeps the saved one
    #[serde(default)]
    token: String,
}

async fn api_save(UrlPath(id): UrlPath<String>, headers: HeaderMap, Json(req): Json<SaveReq>) -> (StatusCode, Json<Value>) {
    if !from_panel(&headers) {
        return (StatusCode::FORBIDDEN, Json(json!({"error": "forbidden"})));
    }
    blocking(move || {
        let e = entry(&id)?;
        if !e.ready {
            return Err("Wait for the server's setup to finish first.".into());
        }
        let wanted = Settings { enabled: req.enabled, domain: req.domain, origins: req.origins, counter: req.counter, mode: req.mode, port: e.web_status.port, own_tunnel: e.web_status.own_tunnel };
        let mut s = validate(wanted, &e, &servers::load())?;
        let token = (!req.token.trim().is_empty()).then(|| clean_token(&req.token)).transpose()?;
        if s.tunnel() {
            // a new token: the app's own tunnel. Otherwise the one it has, or the machine's own
            s.own_tunnel = token.is_some() || (e.web_status.own_tunnel && read_token(&id).is_some());
            if s.enabled && !s.own_tunnel {
                let r = e.remote.as_ref().ok_or("Website status is for servers on a Linux machine.")?;
                if existing_tunnel(r)? != Some("running") {
                    return Err("There's no Cloudflare Tunnel running on this machine. Paste the tunnel's token from Cloudflare.".into());
                }
            }
        } else {
            s.own_tunnel = false;
        }
        if let Some(t) = &token {
            save_token(&id, t)?;
        }
        let store = |s: &Settings| {
            servers::edit(|l| {
                if let Some(x) = l.iter_mut().find(|x| x.id == id) {
                    x.web_status = s.clone();
                }
            })
        };
        if s.enabled {
            // saved first: a setup that stops part way can still be turned off
            store(&s)?;
            apply(&entry(&id)?, true)?;
        } else {
            // taken off the machine first: it only shows as off once it is
            if e.web_status.enabled {
                apply(&e, false)?;
            }
            store(&s)?;
        }
        // a port that had to move: a tunnel's hostname in Cloudflare must follow it
        let moved = e.web_status.port.is_some_and(|p| Some(p) != s.port);
        Ok(json!({"ok": true, "port": s.port, "port_moved": moved}))
    })
    .await
}

pub fn routes() -> Router<Shared> {
    Router::new().route("/api/servers/:id/webstatus", route_get(api_status).post(api_save))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn server(id: &str, host: Option<&str>) -> Entry {
        let mut v = json!({"id": id, "name": format!("Server {id}"), "game": "cstrike", "port": 27016, "dir": "/tmp/x", "kind": "crossover", "ready": true});
        if let Some(h) = host {
            v["remote"] = json!({"host": h, "key": "/k"});
        }
        serde_json::from_value(v).unwrap()
    }

    #[test]
    fn domains() {
        assert_eq!(clean_domain(" Status.LonerStrike.com. ").unwrap(), "status.lonerstrike.com");
        assert_eq!(clean_domain("https://status.example.com/").unwrap(), "status.example.com");
        for bad in ["", "localhost", "1.2.3.4", "a..b.com", "-a.com", "a.com:443", "a b.com", "status.example.com/x", "x.123"] {
            assert!(clean_domain(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn origins() {
        assert_eq!(clean_origin("https://LonerStrike.com/").unwrap(), "https://lonerstrike.com");
        assert_eq!(clean_origin("https://www.example.com:8443").unwrap(), "https://www.example.com:8443");
        assert_eq!(clean_origin("http://localhost:5173").unwrap(), "http://localhost:5173");
        assert_eq!(clean_origin("https://*.Loner-Strike.pages.dev/").unwrap(), "https://*.loner-strike.pages.dev");
        for bad in ["http://*.loner-strike.pages.dev", "https://*.dev", "https://*", "https://a.*.com", "https://**.a.com", "lonerstrike.com", "http://lonerstrike.com", "https://example.com/page", "ftp://example.com", "https://", "https://a.com:0", "https://a.com:x", "*"] {
            assert!(clean_origin(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn tokens() {
        let t = "eyJhIjoiMTIzNDU2Nzg5MCIsInQiOiJhYmNkZWYiLCJzIjoiWldGelpXVmxaV1U9In0=";
        assert_eq!(clean_token(t).unwrap(), t);
        assert_eq!(clean_token(&format!("sudo cloudflared service install {t}\n")).unwrap(), t);
        assert!(clean_token("short").is_err() && clean_token(&format!("{t}'; rm -rf /")).is_err());
    }

    #[test]
    fn settings_are_checked() {
        let a = server("a", Some("1.2.3.4"));
        let mut b = server("b", Some("1.2.3.4"));
        b.web_status = Settings { enabled: true, domain: "status.example.com".into(), origins: vec!["https://example.com".into()], port: Some(8090), ..Settings::default() };
        let other_machine = Settings { port: Some(8091), ..b.web_status.clone() };
        let mut c = server("c", Some("5.6.7.8"));
        c.web_status = other_machine;
        let all = vec![a.clone(), b.clone(), c.clone()];
        let on = |d: &str, o: &[&str]| Settings { enabled: true, domain: d.into(), origins: o.iter().map(|x| x.to_string()).collect(), ..Settings::default() };
        let s = validate(on("Live.Example.com", &["https://example.com/", "https://example.com", " "]), &a, &all).unwrap();
        assert_eq!(s.domain, "live.example.com");
        assert_eq!(s.origins, vec!["https://example.com"]);
        assert_eq!(s.port, Some(8091), "8090 is b's on the same machine; c's machine doesn't count");
        assert_eq!(s.mode, "caddy");
        assert!(validate(on("status.example.com", &["https://example.com"]), &a, &all).is_err(), "b uses that name");
        assert!(validate(on("", &["https://example.com"]), &a, &all).is_err());
        assert!(validate(on("live.example.com", &[]), &a, &all).is_err(), "at least one site");
        assert!(validate(Settings { enabled: false, ..on("", &[]) }, &a, &all).is_ok(), "off: nothing needed");
        assert!(validate(on("live.example.com", &["https://example.com"]), &server("l", None), &all).is_err(), "not on a Linux machine");
        // a port once picked is kept
        assert_eq!(validate(Settings { port: Some(8100), ..on("live.example.com", &["https://example.com"]) }, &a, &all).unwrap().port, Some(8100));
        // and one another server on the machine has is moved off
        assert_eq!(validate(Settings { port: Some(8090), ..on("live.example.com", &["https://example.com"]) }, &a, &all).unwrap().port, Some(8091));
        assert_eq!(validate(Settings { mode: "weird".into(), ..on("live.example.com", &["https://example.com"]) }, &a, &all).unwrap().mode, "caddy");
    }

    #[test]
    fn old_servers_load_and_save_without_it() {
        let e = server("a", None);
        assert!(e.web_status.is_unset());
        assert!(!serde_json::to_string(&e).unwrap().contains("web_status"));
    }

    #[test]
    fn suggested_names() {
        assert_eq!(suggested_domain(Some("play.lonerstrike.com:27016")).as_deref(), Some("status.lonerstrike.com"));
        assert_eq!(suggested_domain(Some("lonerstrike.com")).as_deref(), Some("status.lonerstrike.com"));
        assert_eq!(suggested_domain(Some("1.2.3.4:27015")), None);
        assert_eq!(suggested_domain(None), None);
    }

    #[test]
    fn status_unit() {
        let u = status_unit_text("pub", 8091, 27016, &["https://a.com".into(), "https://b.com".into()], true);
        assert!(u.contains("User=lonerstrike-status\n") && u.contains("Group=lonerstrike-status\n"));
        assert!(u.contains("ExecStart=/usr/bin/python3 /opt/lonerstrike-status/status.py --listen 8091 --game 27016 --origins https://a.com,https://b.com --counter /var/lib/lonerstrike-status/pub/visits.txt\n"));
        for line in ["NoNewPrivileges=yes", "ProtectSystem=strict", "ProtectHome=yes", "PrivateTmp=yes", "ReadWritePaths=/var/lib/lonerstrike-status/pub", "RestrictAddressFamilies=AF_INET AF_INET6", "CapabilityBoundingSet=\n", "MemoryMax=64M"] {
            assert!(u.contains(line), "{line}");
        }
        assert!(!u.contains("$HOME") && !u.contains("sudo"));
        let u = status_unit_text("pub", 8090, 27015, &["https://a.com".into()], false);
        assert!(!u.contains("--counter") && u.contains("--origins https://a.com\n"));
    }

    #[test]
    fn tunnel_units() {
        let f = tunnel_unit_text("pub", true);
        assert!(f.contains("LoadCredential=token:/etc/lonerstrike-status/tunnel-pub.token\n"));
        assert!(f.contains("ExecStart=/usr/bin/cloudflared tunnel --no-autoupdate run --token-file %d/token\n"));
        let e = tunnel_unit_text("pub", false);
        assert!(e.contains("EnvironmentFile=/etc/lonerstrike-status/tunnel-pub.env\n") && !e.contains("--token"));
        for u in [&f, &e] {
            assert!(u.contains("DynamicUser=yes") && u.contains("NoNewPrivileges=yes") && u.contains("ProtectHome=yes") && u.contains("CapabilityBoundingSet=\n"));
        }
    }

    #[test]
    fn caddy_block() {
        let c = caddy_site("status.example.com", 8091);
        assert!(c.contains("status.example.com {\n"));
        assert!(c.contains("\theader -Server\n") && c.contains("@status path /status /visit") && c.contains("reverse_proxy 127.0.0.1:8091") && c.contains("respond 404"));
        assert_eq!(c.matches('{').count(), c.matches('}').count());
    }

    #[test]
    fn scripts() {
        let s = Settings { enabled: true, domain: "status.example.com".into(), origins: vec!["https://example.com".into()], counter: true, mode: "caddy".into(), port: Some(8090), own_tunnel: false };
        let sh = install_sh("pub", &s, 8090, 27016, None);
        assert!(!sh.contains('@') || !sh.lines().any(|l| l.contains("@ID@") || l.contains("@UNIT") || l.contains("@PY@") || l.contains("@CADDY")), "every placeholder filled");
        assert!(sh.contains("useradd --system --no-create-home --home-dir /nonexistent --shell /usr/sbin/nologin lonerstrike-status"));
        assert!(sh.contains("sudo tee /opt/lonerstrike-status/status.py >/dev/null <<'LSPY'\n#!/usr/bin/env python3"));
        assert!(sh.contains("def parse_info(data):") && sh.contains("\nLSPY\n"));
        assert!(sh.contains("sudo install -d -o lonerstrike-status -g lonerstrike-status -m 750 /var/lib/lonerstrike-status/pub"));
        assert!(sh.contains("sudo tee /etc/systemd/system/lonerstrike-pub-status.service"));
        assert!(sh.contains("apt_get install -y -qq caddy") && sh.contains("dl.cloudsmith.io/public/caddy/stable"));
        assert!(sh.contains("--dport $p -j ACCEPT") && sh.contains("ufw allow 443/tcp"));
        assert!(sh.contains("sudo tee /etc/caddy/loner-strike.d/pub.caddy") && sh.contains("reverse_proxy 127.0.0.1:8090"));
        assert!(sh.contains("import /etc/caddy/loner-strike.d/*.caddy") && sh.contains("caddy adapt"));
        assert!(sh.contains("sudo systemctl disable --now lonerstrike-pub-tunnel"), "a tunnel from before goes");
        assert!(!sh.contains("cloudflared tunnel run --help"), "no tunnel in Caddy mode");
        assert!(sh.contains("reload-or-restart caddy") && sh.contains("Is status.example.com already used"));

        let t = Settings { mode: "tunnel".into(), ..s };
        let token = "eyJhIjoiMTIzNDU2Nzg5MCIsInQiOiJhYmNkZWYifQ==";
        let sh = install_sh("pub", &t, 8090, 27016, Some(token));
        assert!(sh.contains("https://pkg.cloudflare.com/cloudflare-main.gpg") && sh.contains("https://pkg.cloudflare.com/cloudflared any main"));
        assert!(sh.contains(&format!("<<'LSTOKEN'\n{token}\nLSTOKEN\n")));
        assert_eq!(sh.matches(token).count(), 1, "the token only goes in the root-only file");
        assert!(sh.contains("LoadCredential=token:") && sh.contains("EnvironmentFile="));
        assert!(!sh.contains("apt_get install -y -qq caddy") && !sh.contains("--dport"), "no Caddy, no ports in tunnel mode");
        assert!(sh.contains("sudo rm -f /etc/caddy/loner-strike.d/pub.caddy"), "a Caddy site from before goes");
        assert!(sh.contains("sudo systemctl restart lonerstrike-pub-tunnel"));

        let off = off_sh("pub");
        assert!(off.contains("disable --now lonerstrike-pub-status") && off.contains("disable --now lonerstrike-pub-tunnel"));
        assert!(off.contains("/etc/lonerstrike-status/tunnel-pub.token") && off.contains("/etc/caddy/loner-strike.d/pub.caddy"));
        assert!(!off.contains("/var/lib/lonerstrike-status/pub"), "the count stays");
        assert!(remove_sh("pub").contains("sudo rm -rf /var/lib/lonerstrike-status/pub"));
    }

    /// Every generated script is valid shell (bash -n), in each mode.
    #[test]
    fn scripts_parse() {
        if cfg!(windows) || std::process::Command::new("bash").arg("--version").output().is_err() {
            return;
        }
        let s = Settings { enabled: true, domain: "status.example.com".into(), origins: vec!["https://example.com".into()], counter: true, mode: "caddy".into(), port: Some(8090), own_tunnel: false };
        let t = Settings { mode: "tunnel".into(), own_tunnel: true, ..s.clone() };
        let token = "eyJhIjoiMTIzNDU2Nzg5MCIsInQiOiJhYmNkZWYifQ==";
        for sh in [install_sh("pub", &s, 8090, 27016, None), install_sh("pub", &t, 8090, 27016, Some(token)), install_sh("pub", &t, 8090, 27016, None), off_sh("pub"), remove_sh("pub")] {
            use std::io::Write;
            let mut c = std::process::Command::new("bash").arg("-n").stdin(std::process::Stdio::piped()).stderr(std::process::Stdio::piped()).spawn().unwrap();
            c.stdin.take().unwrap().write_all(sh.as_bytes()).unwrap();
            let out = c.wait_with_output().unwrap();
            assert!(out.status.success(), "{}\n{sh}", String::from_utf8_lossy(&out.stderr));
        }
        let existing = install_sh("pub", &t, 8090, 27016, None);
        assert!(existing.contains("systemctl is-active --quiet cloudflared ||") && !existing.contains("pkg.cloudflare.com") && !existing.contains("caddy adapt"));
        assert!(!existing.contains("disable --now cloudflared") && !off_sh("pub").contains("cloudflared.service"), "the machine's own tunnel is never touched");
    }

    /// The service itself: Python's own syntax check, then its reply parser on sample bytes.
    #[test]
    fn python_service() {
        let py = if cfg!(windows) { "python" } else { "python3" };
        if std::process::Command::new(py).arg("--version").output().is_err() {
            eprintln!("no {py} here: skipping the service's own checks");
            return;
        }
        let dir = std::env::temp_dir().join(format!("ls-webstatus-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("status.py");
        std::fs::write(&file, STATUS_PY).unwrap();
        let out = std::process::Command::new(py).arg("-m").arg("py_compile").arg(&file).output().unwrap();
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        let test = r#"
import importlib.util, json, sys
spec = importlib.util.spec_from_file_location("status", sys.argv[1])
m = importlib.util.module_from_spec(spec); spec.loader.exec_module(m)
new = b"I\x30My \xc3\xa9 server\x00de_dust2\x00cstrike\x00Counter-Strike\x00\x0a\x00\x05\x14\x03d\x6c\x00\x01"
old = b"m127.0.0.1:27015\x00Old \xe9 box\x00cs_office\x00cstrike\x00Counter-Strike\x00\x04\x10\x2fdl\x00\x00\x01\x02"
oldmod = b"m1.2.3.4:27015\x00Modded\x00de_aztec\x00czero\x00CZ\x00\x06\x20\x2fdw\x00\x01http://a\x00http://b\x00\x00\x01\x00\x00\x00\x02\x00\x00\x00\x00\x01\x01\x04"
al = {"https://lonerstrike.com", "https://*.loner-strike.pages.dev", "http://localhost:5173"}
ok = [m.origin_allowed(o, al) for o in ["https://lonerstrike.com", "https://abc123.loner-strike.pages.dev", "https://a.b.loner-strike.pages.dev", "http://localhost:5173"]]
no = [m.origin_allowed(o, al) for o in ["", "https://evil.com", "http://abc.loner-strike.pages.dev", "https://loner-strike.pages.dev", "https://xloner-strike.pages.dev", "https://abc.loner-strike.pages.dev:8443", "https://lonerstrike.com.evil.com"]]
assert all(ok) and not any(no), (ok, no)
print(json.dumps([m.parse_info(new), m.parse_info(old), m.parse_info(oldmod), m.parse_info(b"I\x30cut"), m.parse_info(b"")]))
"#;
        let out = std::process::Command::new(py).arg("-c").arg(test).arg(&file).output().unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        let v: Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(v[0], json!({"name": "My é server", "map": "de_dust2", "players": 5, "bots": 3, "max_players": 20, "online": true}));
        assert_eq!(v[1], json!({"name": "Old é box", "map": "cs_office", "players": 4, "bots": 2, "max_players": 16, "online": true}));
        assert_eq!(v[2], json!({"name": "Modded", "map": "de_aztec", "players": 6, "bots": 4, "max_players": 32, "online": true}));
        assert!(v[3].is_null() && v[4].is_null());
    }
}
