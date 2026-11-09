//! ls-control: Loner-Strike's shared control for a server on a Linux machine.
//!
//! Friends helping run the server (Admins and Moderators) talk to this small service on
//! the server itself, so they can help while the owner's computer is off, and the owner
//! needs no port forwarding. It runs as the game server's own user
//! (lonerstrike-<id>-control.service), listens on 127.0.0.1 only, and is reached through
//! the server's website status front: Caddy or a Cloudflare Tunnel routes
//! https://<domain>/control/… to it (Caddy and Cloudflare take care of HTTPS).
//!
//! Every request goes through the same steps as the owner's own app (app/src-tauri/src/
//! control/mod.rs, `guard`), with the very same rules (control/rules.rs, compiled in here):
//!
//! 1. their key (one per person, made when they accepted the invite; only its hash is kept,
//!    compared in constant time)
//! 2. the same Loner-Strike version on both sides
//! 3. pinned to this one server: a member of another server, or a path naming another
//!    server, is refused
//! 4. `rules::check`: nothing gets through unless their role allows it (deny by default)
//! 5. what went through is done here through the game (RCON on 127.0.0.1 with the panel's
//!    password from panel_rcon.txt, the plugin's commands, the game's own log), or refused
//!    with "do that from the owner's app" when it needs the owner's files or downloads
//! 6. a change goes in the server's activity log
//!
//! The owner's app manages it over SSH with this same program's commands (`ls-control
//! invite …`, printing JSON): there's no owner key over HTTP.
//!
//! Files: ~/loner-strike/control/<id>/ (only this user can read it): config.json (written
//! by the owner's app), store.json (people and invites), log.jsonl (activity).

#[path = "../../src-tauri/src/control/rules.rs"]
mod rules;

#[path = "../../src-tauri/src/control/common.rs"]
mod common;

/// rules.rs asks for `crate::admins::valid_steamid`, its place in the app
mod admins {
    pub use crate::common::valid_steamid;
}

mod api;
mod game;
mod store;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::PathBuf;

/// Loner-Strike's version (build.rs): helpers' apps must have the same one
pub const VERSION: &str = env!("LS_VERSION");

/// A helper's role on the server (the same names and JSON as the app's control::Role).
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Moderator,
    Admin,
}

impl Role {
    pub fn parse(s: &str) -> Option<Role> {
        match s {
            "admin" => Some(Role::Admin),
            "moderator" => Some(Role::Moderator),
            _ => None,
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Role::Admin => "Admin",
            Role::Moderator => "Moderator",
        }
    }
}

/// A server id as the app makes them (and nothing that could leave its folder).
pub fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 48 && id.bytes().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-') && !id.starts_with('-')
}

/// Where one server's things are, under the game user's home.
#[derive(Clone, Debug)]
pub struct Paths {
    pub home: PathBuf,
    pub id: String,
}

impl Paths {
    pub fn new(home: PathBuf, id: &str) -> Result<Paths, String> {
        if !valid_id(id) {
            return Err("That isn't a server id.".into());
        }
        if !home.is_absolute() {
            return Err("HOME isn't set.".into());
        }
        Ok(Paths { home, id: id.to_string() })
    }
    /// This service's own folder for the server
    pub fn control_dir(&self) -> PathBuf {
        self.home.join("loner-strike/control").join(&self.id)
    }
    /// The server itself (Valve's server and the game folders)
    pub fn server_dir(&self) -> PathBuf {
        self.home.join("loner-strike/servers").join(&self.id)
    }
}

/// What the owner's app tells this service about the server (config.json).
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Config {
    pub id: String,
    /// "cstrike" or "czero"
    pub game: String,
    /// the game's UDP port
    pub port: u16,
    /// where helpers' apps reach this service: https://<domain>/control ("" while the
    /// server has no website status front)
    #[serde(default)]
    pub url: String,
    /// "tunnel" behind a Cloudflare Tunnel (whose CF-Connecting-IP is then the visitor's
    /// address), anything else (Caddy) never trusts that header
    #[serde(default)]
    pub front: String,
    /// the server as helpers see it (name, map, address to join...): the owner's app
    /// writes only what it shows helpers, never a password
    #[serde(default)]
    pub view: Value,
}

impl Config {
    pub fn read(p: &Paths) -> Result<Config, String> {
        let t = std::fs::read_to_string(p.control_dir().join("config.json")).map_err(|_| "This server's shared control isn't set up. Set it up again from the owner's app.".to_string())?;
        let c: Config = serde_json::from_str(&t).map_err(|_| "This server's shared control settings are damaged. Set it up again from the owner's app.".to_string())?;
        if c.id != p.id || !matches!(c.game.as_str(), "cstrike" | "czero") || c.port == 0 {
            return Err("This server's shared control settings are for another server.".into());
        }
        if !c.url.is_empty() && !valid_url(&c.url) {
            return Err("This server's shared control address isn't an https:// address.".into());
        }
        Ok(c)
    }
    pub fn name(&self) -> String {
        self.view["name"].as_str().unwrap_or(&self.id).chars().take(64).collect()
    }
}

/// https://<host>/control: what invites carry (a host name, then exactly /control)
pub fn valid_url(u: &str) -> bool {
    let Some(rest) = u.strip_prefix("https://") else { return false };
    let Some(host) = rest.strip_suffix("/control") else { return false };
    !host.is_empty() && host.len() <= 253 && host.contains('.') && host.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')
}

// ---- command line ---------------------------------------------------------------------

const USAGE: &str = "ls-control: Loner-Strike's shared control for a server on this machine.
  ls-control version
  ls-control serve  --server <id> --port <port>          (the service; systemd runs it)
  ls-control invite --server <id> --role admin|moderator [--steam STEAM_0:1:2]
  ls-control list   --server <id> [--log <entries>]
  ls-control role   --server <id> --member <member id> --role admin|moderator
  ls-control remove --server <id> --member <member id>
  ls-control cancel --server <id> --invite <invite id>
Every command but serve prints one line of JSON.";

/// --name value pairs after the command.
fn flags(args: &[String]) -> Result<std::collections::HashMap<String, String>, String> {
    let mut out = std::collections::HashMap::new();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let k = a.strip_prefix("--").ok_or_else(|| format!("Unexpected {a}."))?;
        let v = it.next().ok_or_else(|| format!("--{k} needs a value."))?;
        if v.len() > 300 || v.chars().any(|c| c.is_control()) {
            return Err(format!("--{k} has an odd value."));
        }
        out.insert(k.to_string(), v.clone());
    }
    Ok(out)
}

fn home() -> Result<PathBuf, String> {
    std::env::var_os("HOME").map(PathBuf::from).filter(|h| h.is_absolute()).ok_or_else(|| "HOME isn't set.".to_string())
}

fn run(args: &[String]) -> Result<Value, String> {
    let cmd = args.first().map(String::as_str).unwrap_or("help");
    let f = flags(args.get(1..).unwrap_or(&[]))?;
    if cmd == "version" {
        return Ok(json!({"version": VERSION}));
    }
    if matches!(cmd, "help" | "--help" | "-h") {
        return Err(USAGE.into());
    }
    let id = f.get("server").ok_or("Say which server (--server <id>).")?;
    let paths = Paths::new(home()?, id)?;
    let files = store::Files::new(&paths);
    match cmd {
        "serve" => {
            let port: u16 = f.get("port").and_then(|p| p.parse().ok()).filter(|p| *p >= 1024).ok_or("Say which port to listen on (--port, 1024 or above).")?;
            api::serve(paths, port)?;
            Ok(json!({"ok": true}))
        }
        "invite" => {
            let role = f.get("role").and_then(|r| Role::parse(r)).ok_or("Pick Admin or Moderator.")?;
            let config = Config::read(&paths)?;
            let (iid, text) = files.invite(&config, role, f.get("steam").map(String::as_str).unwrap_or(""), "owner")?;
            Ok(json!({"ok": true, "invite": text, "id": iid}))
        }
        "list" => {
            let n = f.get("log").and_then(|n| n.parse::<usize>().ok()).unwrap_or(200).min(3000);
            files.view(id, n)
        }
        "role" => {
            let role = f.get("role").and_then(|r| Role::parse(r)).ok_or("Pick Admin or Moderator.")?;
            let m = files.set_role(f.get("member").ok_or("Say who (--member).")?, role)?;
            Ok(json!({"ok": true, "member": store::member_view(&m)}))
        }
        "remove" => {
            let gone = files.remove(f.get("member").ok_or("Say who (--member).")?)?;
            Ok(json!({"ok": true, "removed": gone.iter().map(store::member_view).collect::<Vec<_>>()}))
        }
        "cancel" => {
            files.cancel(f.get("invite").ok_or("Say which invite (--invite).")?)?;
            Ok(json!({"ok": true}))
        }
        _ => Err(USAGE.into()),
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(v) => println!("{v}"),
        Err(e) => {
            println!("{}", json!({"error": e}));
            std::process::exit(2);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_and_urls() {
        assert!(valid_id("friday-frags") && valid_id("srv2"));
        for bad in ["", "../x", "a/b", "-x", "A", &"a".repeat(49)] {
            assert!(!valid_id(bad), "{bad}");
        }
        assert!(valid_url("https://status.example.com/control"));
        for bad in ["http://status.example.com/control", "https://status.example.com", "https://x@evil.com/control", "https://status.example.com/control/x", "https:///control", "https://localhost/control"] {
            assert!(!valid_url(bad), "{bad}");
        }
    }

    #[test]
    fn version_is_the_apps() {
        assert_eq!(VERSION.split('.').count(), 3);
        assert_eq!(run(&["version".into()]).unwrap()["version"], VERSION);
    }

    #[test]
    fn command_line() {
        assert!(flags(&["--server".into(), "x".into(), "--role".into(), "admin".into()]).is_ok());
        assert!(flags(&["--server".into()]).is_err() && flags(&["server".into(), "x".into()]).is_err());
        assert!(run(&["invite".into(), "--server".into(), "../etc".into(), "--role".into(), "admin".into()]).is_err());
    }
}
