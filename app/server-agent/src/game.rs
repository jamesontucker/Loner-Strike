//! The game server on this machine: RCON on 127.0.0.1 with the panel's password (the file
//! the plugin and the owner's app share), the service's state, and the few files the
//! helpers' actions read (the console log, the plugin's status file, the map lists).
//! Behind a trait so the guard can be tested without a game.

use crate::common;
use serde_json::{json, Value};
use std::net::UdpSocket;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

#[derive(Debug, PartialEq)]
pub enum RconError {
    /// the game isn't running (or not answering)
    Offline,
    /// it didn't take the panel's password
    BadPassword,
}

pub const OFFLINE: &str = "The server isn't running.";
pub const BAD_PASSWORD: &str = "The game didn't accept Loner-Strike's console password. Restart the map once.";

impl RconError {
    pub fn say(&self) -> String {
        match self {
            RconError::Offline => OFFLINE.into(),
            RconError::BadPassword => BAD_PASSWORD.into(),
        }
    }
}

pub trait GameIo: Send + Sync {
    /// one console command, its reply
    fn rcon(&self, cmd: &str) -> Result<String, RconError>;
    /// (the service running, seconds up)
    fn active(&self) -> (bool, u64);
    /// the game folder (…/servers/<id>/<game>)
    fn game_dir(&self) -> PathBuf;
}

/// The plugin's data folder in a game folder
pub fn data_dir(game_dir: &Path) -> PathBuf {
    game_dir.join("addons/amxmodx/data/gamemodes")
}

/// The panel's RCON password as it is now (panel_rcon.txt, shared with the plugin).
pub fn rcon_password(game_dir: &Path) -> Option<String> {
    std::fs::read_to_string(data_dir(game_dir).join("panel_rcon.txt")).ok().map(|p| p.trim().to_string()).filter(|p| !p.is_empty())
}

/// The join password as the server has it now (friends.json in the plugin's data folder)
pub fn join_password(game_dir: &Path) -> Option<String> {
    let t = std::fs::read_to_string(data_dir(game_dir).join("friends.json")).ok()?;
    serde_json::from_str::<Value>(&t).ok()?["password"].as_str().map(String::from).filter(|p| !p.is_empty())
}

/// Secrets that never go to a helper, as they are now: the panel's RCON password and the
/// join password (both kept in the plugin's data folder).
pub fn secrets(game_dir: &Path) -> Vec<String> {
    let mut out: Vec<String> = rcon_password(game_dir).into_iter().collect();
    out.extend(join_password(game_dir));
    // (short ones would blank out ordinary words)
    out.retain(|s| s.len() >= 4);
    out
}

/// Text with the secrets blanked out.
pub fn redact(text: &str, secrets: &[String]) -> String {
    let mut t = text.to_string();
    for s in secrets {
        if t.contains(s.as_str()) {
            t = t.replace(s.as_str(), "••••");
        }
    }
    t
}

pub struct Real {
    pub game_dir: PathBuf,
    pub port: u16,
    pub unit: String,
}

impl GameIo for Real {
    fn rcon(&self, cmd: &str) -> Result<String, RconError> {
        // read each time: the owner's app can change it
        let pw = std::fs::read_to_string(data_dir(&self.game_dir).join("panel_rcon.txt")).map_err(|_| RconError::BadPassword)?;
        rcon_udp(self.port, pw.trim(), cmd)
    }

    fn active(&self) -> (bool, u64) {
        let out = |args: &[&str]| std::process::Command::new("systemctl").args(args).output().ok().map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).unwrap_or_default();
        if out(&["is-active", &self.unit]) != "active" {
            return (false, 0);
        }
        let pid = out(&["show", "-p", "MainPID", "--value", &self.unit]);
        let secs = pid.parse::<u32>().ok().filter(|p| *p > 0).and_then(|p| {
            let o = std::process::Command::new("ps").args(["-o", "etimes=", "-p", &p.to_string()]).output().ok()?;
            String::from_utf8_lossy(&o.stdout).trim().parse::<u64>().ok()
        });
        (true, secs.unwrap_or(0))
    }

    fn game_dir(&self) -> PathBuf {
        self.game_dir.clone()
    }
}

/// GoldSrc RCON: a challenge, then the command with the password; the answer may come in
/// several packets, or split ones (as the app's lsrcon helper reads it).
pub fn rcon_udp(port: u16, password: &str, cmd: &str) -> Result<String, RconError> {
    // a password or command that could end the line or its quotes is never sent
    if password.is_empty() || password.contains('"') || password.chars().any(|c| c.is_control()) || cmd.chars().any(|c| c.is_control()) {
        return Err(RconError::BadPassword);
    }
    const P: &[u8] = b"\xff\xff\xff\xff";
    let s = UdpSocket::bind(("127.0.0.1", 0)).map_err(|_| RconError::Offline)?;
    s.connect(("127.0.0.1", port)).map_err(|_| RconError::Offline)?;
    s.set_read_timeout(Some(Duration::from_millis(1500))).map_err(|_| RconError::Offline)?;
    let mut buf = vec![0u8; 65535];
    s.send(&[P, b"challenge rcon\n"].concat()).map_err(|_| RconError::Offline)?;
    let n = s.recv(&mut buf).map_err(|_| RconError::Offline)?;
    let text = String::from_utf8_lossy(&buf[4.min(n)..n]).to_string();
    let challenge = text.split_whitespace().nth(2).filter(|c| c.bytes().all(|b| b.is_ascii_digit())).ok_or(RconError::Offline)?.to_string();
    let mut line = [P, b"rcon ", challenge.as_bytes(), b" \"", password.as_bytes(), b"\" "].concat();
    // console text is Latin-1
    line.extend(cmd.chars().map(|c| if (c as u32) < 256 { c as u8 } else { b'?' }));
    line.push(b'\n');
    s.send(&line).map_err(|_| RconError::Offline)?;
    let mut singles: Vec<Vec<u8>> = Vec::new();
    // split answers: id -> (parts, total)
    let mut splits: std::collections::BTreeMap<[u8; 4], (std::collections::BTreeMap<u8, Vec<u8>>, u8)> = Default::default();
    let mut got_any = false;
    let started = Instant::now();
    loop {
        match s.recv(&mut buf) {
            Ok(n) => {
                got_any = true;
                let d = &buf[..n];
                if n > 9 && &d[..4] == b"\xfe\xff\xff\xff" {
                    let id = [d[4], d[5], d[6], d[7]];
                    let e = splits.entry(id).or_default();
                    e.0.insert(d[8] >> 4, d[9..].to_vec());
                    e.1 = d[8] & 15;
                } else {
                    singles.push(d.to_vec());
                }
                let waiting = splits.values().any(|(parts, total)| parts.len() < *total as usize);
                let _ = s.set_read_timeout(Some(Duration::from_millis(if waiting { 400 } else { 50 })));
            }
            Err(_) => break,
        }
        // never more than a few seconds, whatever comes in
        if started.elapsed() > Duration::from_secs(5) {
            break;
        }
    }
    if !got_any {
        return Err(RconError::Offline);
    }
    for (_, (parts, _)) in splits {
        singles.push(parts.into_values().flatten().collect());
    }
    let out: String = singles.iter().map(|d| if d.len() > 5 && &d[..4] == P { &d[5..] } else { &d[..] }).flat_map(|d| d.iter().map(|&b| b as char)).collect();
    if out.contains("Bad rcon_password") {
        return Err(RconError::BadPassword);
    }
    Ok(out)
}

// ---- what helpers see ------------------------------------------------------------------

/// The plugin's live state (gm_status), as the app's game.rs reads it: the plugin writes
/// status.json (here, on this machine) and says so, or answers in "GM " lines.
pub fn state(io: &dyn GameIo) -> Value {
    let reply = match io.rcon("gm_status") {
        Ok(r) => r,
        Err(RconError::BadPassword) => return json!({"online": false, "reason": BAD_PASSWORD}),
        Err(RconError::Offline) => return json!({"online": false, "reason": OFFLINE}),
    };
    let path = data_dir(&io.game_dir()).join("status.json");
    let lines: Vec<String> = if reply.contains("GM status written") && path.is_file() {
        std::fs::read(&path).unwrap_or_default().iter().map(|&b| b as char).collect::<String>().lines().map(String::from).collect()
    } else {
        reply.lines().filter_map(|l| l.strip_prefix("GM ").map(String::from)).collect()
    };
    if lines.is_empty() {
        return json!({"online": false, "reason": "The game is running but the Game Mode Menu plugin didn't answer."});
    }
    let mut out = json!({"online": true, "modes": [], "settings": [], "addons": [], "addonsets": [], "players": [], "presets": [], "events": [], "me": {}});
    for line in lines {
        let Ok(Value::Object(mut obj)) = serde_json::from_str::<Value>(&line) else { continue };
        let kind = obj.remove("t").and_then(|v| v.as_str().map(String::from)).unwrap_or_default();
        match kind.as_str() {
            "state" => out["state"] = Value::Object(obj),
            "me" => out["me"] = Value::Object(obj),
            "mode" | "setting" | "addon" | "addonset" | "player" | "preset" | "event" => {
                if let Some(list) = out[format!("{kind}s")].as_array_mut() {
                    list.push(Value::Object(obj));
                }
            }
            _ => {}
        }
    }
    out
}

/// The last lines of the server's console (qconsole.log), without the panel's own checks
/// and with passwords blanked, as the owner's app shows it.
pub fn console(game_dir: &Path, secrets: &[String]) -> Vec<String> {
    use std::io::{Read, Seek, SeekFrom};
    let mut lines = Vec::new();
    for p in [game_dir.join("qconsole.log"), game_dir.parent().map(|d| d.join("qconsole.log")).unwrap_or_default()] {
        let Ok(mut f) = std::fs::File::open(&p) else { continue };
        let len = f.metadata().map(|m| m.len()).unwrap_or(0);
        let _ = f.seek(SeekFrom::Start(len.saturating_sub(48 * 1024)));
        let mut buf = Vec::new();
        let _ = f.read_to_end(&mut buf);
        lines = buf.iter().map(|&b| b as char).collect::<String>().lines().map(String::from).collect();
        if !lines.is_empty() {
            break;
        }
    }
    let mut lines = common::without_polls(lines, "127.0.0.1", &|_| false);
    let skip = lines.len().saturating_sub(200);
    lines.split_off(skip).into_iter().map(|l| redact(&common::hide_passwords(&l), secrets)).collect()
}

/// The maps on the server (maps/*.bsp in the game folder, and cstrike's for Condition Zero).
pub fn maps(game_dir: &Path) -> Vec<Value> {
    let mut found: std::collections::BTreeMap<String, (String, bool)> = Default::default();
    let mut dirs = vec![game_dir.to_path_buf()];
    if let Some(cs) = game_dir.parent().map(|d| d.join("cstrike")).filter(|d| d != game_dir) {
        dirs.push(cs);
    }
    for d in dirs {
        let cz = d.file_name().is_some_and(|n| n == "czero");
        for e in std::fs::read_dir(d.join("maps")).into_iter().flatten().flatten() {
            let f = e.file_name().to_string_lossy().to_string();
            let low = f.to_lowercase();
            if let Some(stem) = low.strip_suffix(".bsp") {
                if common::safe_arg(stem) {
                    let entry = found.entry(stem.to_string()).or_insert_with(|| (f[..f.len() - 4].to_string(), false));
                    entry.1 |= cz;
                }
            }
        }
    }
    found.into_values().map(|(name, cz)| json!({"name": name, "cz": cz, "thumb": null, "fav": false, "hidden": false, "recent": -1})).collect()
}

/// A mode's map list (configs/gamemodes/maps/<mode>.txt).
pub fn mode_maps(game_dir: &Path, mode: &str) -> Vec<String> {
    std::fs::read_to_string(game_dir.join("addons/amxmodx/configs/gamemodes/maps").join(format!("{mode}.txt")))
        .unwrap_or_default()
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty() && !l.starts_with(';') && !l.starts_with("//") && common::safe_arg(l))
        .map(String::from)
        .collect()
}

/// Whether a SteamID is someone a Moderator doesn't ban or mute: the server's owner, Admins
/// or Moderators in the game (admins.json, as the owner's app keeps it), or `helpers` (the
/// SteamIDs of people helping run it). Can't tell: as if so.
pub fn outranks_moderator(game_dir: &Path, steamid: &str, helpers: &[String]) -> bool {
    if helpers.iter().any(|h| !h.is_empty() && common::same_steamid(h, steamid)) {
        return true;
    }
    let file = data_dir(game_dir).join("admins.json");
    let a: Value = match std::fs::read_to_string(&file) {
        Ok(t) => match serde_json::from_str(&t) {
            Ok(v) => v,
            Err(_) => return true,
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return false,
        Err(_) => return true,
    };
    let today = today();
    a["people"].as_array().into_iter().flatten().any(|p| {
        let expired = p["expires"].as_str().is_some_and(|e| e < today.as_str());
        p["id"].as_str().is_some_and(|id| common::same_steamid(id, steamid)) && matches!(p["role"].as_str(), Some("owner" | "admin" | "moderator")) && !expired
    })
}

/// YYYY-MM-DD (UTC)
fn today() -> String {
    let s = crate::store::stamp();
    format!("{}-{}-{}", &s[0..4], &s[4..6], &s[6..8])
}

/// The ban file for a kind of ban (in the game folder): banned.cfg or listip.cfg
pub fn ban_file(kind: &str) -> &'static str {
    if kind == "ip" {
        "listip.cfg"
    } else {
        "banned.cfg"
    }
}

/// A ban file with a ban for good added (once), as the app's admins.rs writes it.
pub fn with_ban(text: &str, id: &str) -> String {
    let has = text.lines().any(|l| {
        let t: Vec<&str> = l.split_whitespace().collect();
        t.len() >= 3 && t[0].eq_ignore_ascii_case("banid") && t[2] == id
    });
    if has {
        return text.to_string();
    }
    let nl = if text.contains("\r\n") || text.is_empty() { "\r\n" } else { "\n" };
    let base = text.trim_end_matches(['\r', '\n']);
    format!("{base}{}banid 0.0 {id}{nl}", if base.is_empty() { "" } else { nl })
}

/// A ban file without the lines for `value` (the app's admins::remove_ban_line).
pub fn without_ban(text: &str, value: &str) -> String {
    let nl = if text.contains("\r\n") { "\r\n" } else { "\n" };
    let kept: Vec<&str> = text.lines().filter(|l| l.split_whitespace().nth(2) != Some(value)).collect();
    if kept.is_empty() {
        String::new()
    } else {
        kept.join(nl) + nl
    }
}

/// A mute that's there now (gags.txt: SteamID, until, c/v flags, name): (until, chat, voice)
pub fn current_gag(game_dir: &Path, id: &str) -> Option<(u64, bool, bool)> {
    let t: String = std::fs::read(data_dir(game_dir).join("gags.txt")).ok()?.iter().map(|&b| b as char).collect();
    let now = crate::store::now();
    t.lines().filter(|l| !l.starts_with(';')).find_map(|l| {
        let p: Vec<&str> = l.split('\t').collect();
        if p.len() < 3 || !p[0].trim().eq_ignore_ascii_case(id) {
            return None;
        }
        let until: u64 = p[1].trim().parse().ok()?;
        (until == 0 || until > now).then(|| (until, p[2].contains('c'), p[2].contains('v')))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ban_files() {
        assert_eq!(with_ban("", "STEAM_0:1:1"), "banid 0.0 STEAM_0:1:1\r\n");
        assert_eq!(with_ban("banid 0.0 STEAM_0:1:1\n", "STEAM_0:1:1"), "banid 0.0 STEAM_0:1:1\n");
        assert_eq!(with_ban("banid 0.0 STEAM_0:1:1\n", "STEAM_0:1:2"), "banid 0.0 STEAM_0:1:1\nbanid 0.0 STEAM_0:1:2\n");
        assert_eq!(without_ban("banid 0.0 STEAM_0:1:1\nbanid 0.0 STEAM_0:1:2\n", "STEAM_0:1:1"), "banid 0.0 STEAM_0:1:2\n");
    }

    #[test]
    fn secrets_are_blanked() {
        let s = vec!["Abc123xyz".to_string()];
        assert_eq!(redact("rcon_password Abc123xyz now", &s), "rcon_password •••• now");
        let dir = std::env::temp_dir().join(format!("ls-control-game-{}", crate::store::random_hex(4)));
        std::fs::create_dir_all(data_dir(&dir)).unwrap();
        std::fs::write(data_dir(&dir).join("panel_rcon.txt"), "SecretRcon123\n").unwrap();
        std::fs::write(data_dir(&dir).join("friends.json"), r#"{"name":"x","password":"joinme"}"#).unwrap();
        std::fs::write(dir.join("qconsole.log"), "Rcon from 127.0.0.1:1:\nrcon 1 \"SecretRcon123\" gm_status\nhello joinme\nrcon 2 \"SecretRcon123\" say hi\n").unwrap();
        let lines = console(&dir, &secrets(&dir));
        assert_eq!(lines, vec!["hello ••••".to_string(), "rcon 2 \"••••\" say hi".to_string()]);
        // the owner and the game's own admins are above a Moderator
        std::fs::write(data_dir(&dir).join("admins.json"), r#"{"people":[{"id":"STEAM_0:1:7","role":"admin"},{"id":"STEAM_0:1:8","role":"vip"},{"id":"STEAM_0:1:9","role":"moderator","expires":"2001-01-01"}]}"#).unwrap();
        assert!(outranks_moderator(&dir, "steam_0:1:7", &[]));
        assert!(!outranks_moderator(&dir, "STEAM_0:1:8", &[]) && !outranks_moderator(&dir, "STEAM_0:1:9", &[]));
        assert!(outranks_moderator(&dir, "STEAM_0:1:8", &["STEAM_0:1:8".into()]), "someone helping run it");
        std::fs::write(data_dir(&dir).join("admins.json"), "{damaged").unwrap();
        assert!(outranks_moderator(&dir, "STEAM_0:1:8", &[]), "can't tell: as if so");
        std::fs::write(data_dir(&dir).join("gags.txt"), "; header\nSTEAM_0:1:3\t0\tc\tBob\n").unwrap();
        assert_eq!(current_gag(&dir, "STEAM_0:1:3"), Some((0, true, false)));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
