//! Community roles on a server on a Linux machine (docs/notes/Roles.md): what the owner's app
//! (app/src-tauri/src/roles.rs) does for a server on its own computer, done here so it works
//! while the owner's computer is off.
//!
//! - the /link relay: the game's log is watched for gm_extras' `triggered "ls_link"` lines; the
//!   list service makes the code, the plugin shows it (`ls_link_show` / `ls_link_fail` over RCON)
//! - supporters: every few minutes, official servers get every supporter's SteamID and others
//!   (with "Honor Loner-Strike supporters" on) the supporters among the players on now; they go in
//!   their own block of users.ini (flag "s"), then `amx_reloadadmins`
//! - moderator invites made for an application (`ls-control invite --app N`, or a shared-control
//!   Admin through the API): redeeming and removing them are told to the list service
//!
//! The list service is reached with curl (no TLS code in here): its settings, the server token
//! among them, go in on stdin (`curl -K -`), never on the command line. The token is the file
//! roles-token in this service's folder (0600, written by the owner's app).
//!
//! This runs as its own unit (`ls-control roles`, lonerstrike-<id>-control-roles.service): the
//! shared control service itself may only talk to this machine (IPAddressAllow=localhost), so
//! what an Admin asks for through it goes here through a spool folder in the service's own
//! folder (roles-spool/<id>.req, answered in <id>.resp), and only these calls are passed on.

use crate::{Config, Paths};
use serde_json::{json, Value};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// Set in the shared control service (no internet there): calls go through the spool.
static SPOOLED: AtomicBool = AtomicBool::new(false);

pub fn use_spool() {
    SPOOLED.store(true, Ordering::SeqCst);
}

fn spool_dir(paths: &Paths) -> PathBuf {
    paths.control_dir().join("roles-spool")
}

/// Only these go through the spool: an Admin's applications, invites and revokes.
fn spool_allows(method: &str, path: &str) -> bool {
    let seg: Vec<&str> = path.split('/').collect();
    match (method, seg.as_slice()) {
        ("GET", ["applications"]) => true,
        ("POST", ["applications", n, "invite" | "revoke" | "redeemed"]) => !n.is_empty() && n.len() <= 12 && n.chars().all(|c| c.is_ascii_digit()),
        _ => false,
    }
}

/// A call from the shared control service, answered by the roles unit (within 25 s).
fn via_spool(paths: &Paths, method: &str, path: &str, body: Option<&Value>) -> Result<Value, String> {
    if !spool_allows(method, path) {
        return Err("That can't be asked from here.".into());
    }
    let dir = spool_dir(paths);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let id = crate::store::random_hex(8);
    let req = dir.join(format!("{id}.req"));
    let tmp = dir.join(format!("{id}.tmp"));
    std::fs::write(&tmp, json!({"method": method, "path": path, "body": body}).to_string()).and_then(|_| std::fs::rename(&tmp, &req)).map_err(|e| e.to_string())?;
    let resp = dir.join(format!("{id}.resp"));
    let started = Instant::now();
    while started.elapsed() < Duration::from_secs(25) {
        std::thread::sleep(Duration::from_millis(250));
        if let Ok(text) = std::fs::read_to_string(&resp) {
            let _ = std::fs::remove_file(&resp);
            let v: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
            return match v["error"].as_str() {
                Some(e) => Err(e.to_string()),
                None => Ok(v["ok"].clone()),
            };
        }
    }
    let _ = std::fs::remove_file(&req);
    Err("The server's community roles helper didn't answer. The owner can update the server's files (Shared control) to start it again.".into())
}

/// The roles unit answering what the shared control service spooled.
fn answer_spool(paths: &Paths) {
    let dir = spool_dir(paths);
    let Ok(rd) = std::fs::read_dir(&dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        if p.extension().and_then(|x| x.to_str()) != Some("req") {
            // answers nobody picked up, and unfinished requests, go after a minute
            if e.metadata().and_then(|m| m.modified()).ok().and_then(|t| t.elapsed().ok()).is_some_and(|age| age > Duration::from_secs(60)) {
                let _ = std::fs::remove_file(&p);
            }
            continue;
        }
        let text = std::fs::read_to_string(&p).unwrap_or_default();
        let _ = std::fs::remove_file(&p);
        let v: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
        let (method, path) = (v["method"].as_str().unwrap_or(""), v["path"].as_str().unwrap_or(""));
        let out = if spool_allows(method, path) {
            match call(paths, method, path, v.get("body").filter(|b| !b.is_null())) {
                Ok(ok) => json!({"ok": ok}),
                Err(e) => json!({"error": e}),
            }
        } else {
            json!({"error": "That can't be asked from here."})
        };
        let resp = p.with_extension("resp");
        let tmp = p.with_extension("rtmp");
        let _ = std::fs::write(&tmp, out.to_string()).and_then(|_| std::fs::rename(&tmp, &resp));
    }
}

const LIST: &str = "https://list.lonerstrike.com";
const UNCLAIMED: &str = "This server's claim ended. Claim it again in the owner's Loner-Strike.";
const VIP_EVERY: Duration = Duration::from_secs(300);

pub fn token(paths: &Paths) -> Option<String> {
    let t = std::fs::read_to_string(paths.control_dir().join("roles-token")).ok()?;
    let t = t.trim().to_string();
    (t.starts_with("lsr1_") && t.len() <= 80 && t[5..].chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')).then_some(t)
}

/// A curl config quoted string ("..." with \ and " escaped).
fn quoted(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', "\\n").replace('\r', "\\r"))
}

/// What curl reads on stdin for one call (`curl -K -`).
pub fn curl_config(url: &str, method: &str, token: &str, body: Option<&Value>) -> String {
    let mut c = format!("url = {}\nrequest = {}\nheader = {}\nheader = \"Content-Type: application/json\"\nmax-time = 20\nsilent\nshow-error\nwrite-out = \"\\n%{{http_code}}\"\n",
        quoted(url), quoted(method), quoted(&format!("Authorization: Bearer {token}")));
    if let Some(b) = body {
        c.push_str(&format!("data = {}\n", quoted(&b.to_string())));
    }
    c
}

/// One call to the list service's /roles/…: its JSON, or its words when it says no.
pub fn call(paths: &Paths, method: &str, path: &str, body: Option<&Value>) -> Result<Value, String> {
    let tok = token(paths).ok_or(UNCLAIMED)?;
    if SPOOLED.load(Ordering::SeqCst) {
        return via_spool(paths, method, path, body);
    }
    let mut child = Command::new("curl")
        .args(["-K", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| "curl isn't installed on the server.".to_string())?;
    let sent = match (method, body) {
        ("GET", _) => None,
        (_, Some(b)) => Some(b.clone()),
        (_, None) => Some(json!({})),
    };
    let config = curl_config(&format!("{LIST}/roles/{path}"), method, &tok, sent.as_ref());
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(config.as_bytes());
    }
    let out = child.wait_with_output().map_err(|_| "curl didn't run.".to_string())?;
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    let (body, status) = text.rsplit_once('\n').unwrap_or(("", text.as_str()));
    let status: u16 = status.trim().parse().unwrap_or(0);
    let v: Value = serde_json::from_str(body).unwrap_or(Value::Null);
    if status == 0 {
        return Err("Couldn't reach Loner-Strike's list service.".into());
    }
    if status == 401 {
        return Err(UNCLAIMED.into());
    }
    if !(200..300).contains(&status) {
        return Err(v["error"].as_str().map(str::to_string).unwrap_or_else(|| format!("The list service said no (status {status}).")));
    }
    Ok(v)
}

/// A line gm_extras added to lslink/requests.txt, "<unix time> <userid> <SteamID> <name>": (name, uid,
/// SteamID), or None when it isn't one or is older than half a minute (a relay that just started
/// doesn't answer old ones). Only the plugin writes that file: a player can't make one up.
pub fn link_request(line: &str, now: u64) -> Option<(String, u32, String)> {
    let mut parts = line.trim_end_matches(['\r', '\n']).splitn(4, ' ');
    let t = parts.next()?.parse::<u64>().ok()?;
    let uid = parts.next()?.parse::<u32>().ok()?;
    let steam = parts.next()?.trim().to_uppercase();
    let name = parts.next()?;
    if now.saturating_sub(t) > 30 || t > now + 30 || !crate::common::valid_steamid(&steam) || name.is_empty() || name.len() > 64 {
        return None;
    }
    Some((name.to_string(), uid, steam))
}

/// The folder the plugin and the relay talk through (nothing goes over RCON, so no console
/// password lands in the console log).
pub fn lslink_dir(game_dir: &std::path::Path) -> std::path::PathBuf {
    game_dir.join("addons/amxmodx/data/gamemodes/lslink")
}

/// Answers for the plugin, "<userid> <SteamID> show <code>" or "<userid> <SteamID> fail <word>":
/// written as answers.txt in one go when the plugin has taken the last one (it reads and removes
/// it); until then they wait in `pending`.
pub fn deliver(dir: &std::path::Path, pending: &mut Vec<String>) {
    if pending.is_empty() || dir.join("answers.txt").exists() {
        return;
    }
    let tmp = dir.join("answers.txt.new");
    let text = pending.join("\n") + "\n";
    if std::fs::write(&tmp, text).and_then(|_| std::fs::rename(&tmp, dir.join("answers.txt"))).is_ok() {
        pending.clear();
    }
}

fn now_secs() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// A code is plain: XXXX-XXXX.
fn plain_code(c: &str) -> bool {
    c.len() == 9 && c.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

fn fail_word(err: &str) -> &'static str {
    if err.contains("don't seem to be on") {
        "away"
    } else if err.contains("Too many") || err.contains("asked for a few") || err.contains("players today") {
        "limit"
    } else if err == UNCLAIMED {
        "unclaimed"
    } else if err.contains("SteamID") {
        "steamid"
    } else {
        "error"
    }
}

pub const SUPPORTERS_BEGIN: &str = "; ---- Loner-Strike supporters (from the Loner-Strike Discord; rewritten automatically) ----";
pub const SUPPORTERS_END: &str = "; ---- end of Loner-Strike supporters ----";

/// users.ini with the supporters' block set to `ids` (the app's admins.rs does the same).
pub fn with_supporters(text: &str, ids: &[String]) -> String {
    let nl = if text.contains("\r\n") || text.is_empty() { "\r\n" } else { "\n" };
    let mut ids: Vec<String> = ids.iter().map(|s| s.trim().to_uppercase()).filter(|s| crate::common::valid_steamid(s)).collect();
    ids.sort();
    ids.dedup();
    let block = if ids.is_empty() {
        String::new()
    } else {
        let mut out = vec![SUPPORTERS_BEGIN.to_string()];
        out.extend(ids.iter().map(|id| format!("\"{id}\" \"\" \"s\" \"ce\"")));
        out.push(SUPPORTERS_END.to_string());
        out.join(nl)
    };
    let rest = match (text.find(SUPPORTERS_BEGIN), text.find(SUPPORTERS_END)) {
        (Some(s), Some(e)) if e > s => format!("{}{}", text[..s].trim_end_matches(['\r', '\n']), &text[e + SUPPORTERS_END.len()..]),
        _ => text.to_string(),
    };
    let base = rest.trim_end_matches(['\r', '\n']);
    match (base.is_empty(), block.is_empty()) {
        (true, true) => String::new(),
        (true, false) => format!("{block}{nl}"),
        (false, true) => format!("{base}{nl}"),
        (false, false) => format!("{base}{nl}{nl}{block}{nl}"),
    }
}

pub fn status_steamids(status: &str) -> Vec<String> {
    let mut out: Vec<String> = status.split(|c: char| c.is_whitespace() || c == '"').map(|w| w.trim().to_uppercase()).filter(|w| crate::common::valid_steamid(w)).collect();
    out.sort();
    out.dedup();
    out
}

/// The SteamIDs gm_extras says are on the server now (lslink/players.txt, written every 10 s; an
/// older file counts as nobody).
pub fn players_here(game_dir: &Path) -> Vec<String> {
    let p = lslink_dir(game_dir).join("players.txt");
    let fresh = std::fs::metadata(&p).and_then(|m| m.modified()).ok().and_then(|t| t.elapsed().ok()).is_some_and(|age| age < Duration::from_secs(60));
    if !fresh {
        return vec![];
    }
    status_steamids(&std::fs::read_to_string(&p).unwrap_or_default())
}

fn read_new(path: &Path, offset: &mut u64) -> Vec<String> {
    let Ok(mut f) = std::fs::File::open(path) else { return vec![] };
    let len = f.metadata().map(|m| m.len()).unwrap_or(0);
    if len < *offset {
        *offset = 0;
    }
    if f.seek(SeekFrom::Start(*offset)).is_err() {
        return vec![];
    }
    let mut buf = Vec::new();
    let _ = f.take(256 * 1024).read_to_end(&mut buf);
    let Some(end) = buf.iter().rposition(|b| *b == b'\n') else { return vec![] };
    *offset += end as u64 + 1;
    String::from_utf8_lossy(&buf[..end]).lines().map(str::to_string).collect()
}

fn supporters(paths: &Paths, game_dir: &Path) -> Result<(), String> {
    let v = call(paths, "GET", "server", None)?;
    let ids: Vec<String> = if v["official"].as_bool() == Some(true) {
        call(paths, "GET", "vip", None)?["steamids"].as_array().into_iter().flatten().filter_map(|s| s.as_str().map(str::to_string)).collect()
    } else if v["settings"]["honor_supporters"].as_bool() == Some(true) {
        let on: Vec<String> = players_here(game_dir).into_iter().take(64).collect();
        if on.is_empty() {
            vec![]
        } else {
            call(paths, "POST", "vip/check", Some(&json!({"steamids": on})))?["steamids"].as_array().into_iter().flatten().filter_map(|s| s.as_str().map(str::to_string)).collect()
        }
    } else {
        vec![]
    };
    let users = game_dir.join("addons/amxmodx/configs/users.ini");
    if !users.parent().is_some_and(|d| d.is_dir()) {
        return Ok(());
    }
    let old = std::fs::read(&users).map(|b| b.iter().map(|&c| c as char).collect::<String>()).unwrap_or_default();
    let new = with_supporters(&old, &ids);
    if new != old {
        let bytes: Vec<u8> = new.chars().map(|c| if (c as u32) < 256 { c as u8 } else { b'?' }).collect();
        // in place: the roles unit may write this one file, not the folder (a link is never followed)
        if std::fs::symlink_metadata(&users).is_ok_and(|m| m.file_type().is_symlink()) {
            return Err("users.ini is a link; left alone".into());
        }
        std::fs::write(&users, bytes).map_err(|e| format!("users.ini: {e}"))?;
        // gm_extras reloads admins when it sees this (no RCON: it would put the password in the console log)
        let _ = std::fs::write(lslink_dir(game_dir).join("reload-admins"), b"1\n");
    }
    Ok(())
}

/// The roles unit: the relay, the supporters check and the spool, for as long as it runs.
pub fn run(paths: Paths) {
    {
        // the requests file's offset, and answers waiting for the plugin
        let mut req_offset: u64 = 0;
        let mut pending: Vec<String> = Vec::new();
        let mut vip_at: Option<Instant> = None;
        loop {
            std::thread::sleep(Duration::from_secs(1));
            answer_spool(&paths);
            if token(&paths).is_none() {
                continue;
            }
            let Ok(config) = Config::read(&paths) else { continue };
            let game_dir = paths.server_dir().join(&config.game);
            let dir = lslink_dir(&game_dir);
            for line in read_new(&dir.join("requests.txt"), &mut req_offset) {
                let Some((name, uid, steam)) = link_request(&line, now_secs()) else { continue };
                match call(&paths, "POST", "link/code", Some(&json!({"steamid": steam, "name": name}))) {
                    Ok(v) => {
                        if let Some(code) = v["code"].as_str().filter(|c| plain_code(c)) {
                            pending.push(format!("{uid} {steam} show {code}"));
                        }
                    }
                    Err(e) => pending.push(format!("{uid} {steam} fail {}", fail_word(&e))),
                }
            }
            deliver(&dir, &mut pending);
            if vip_at.map_or(true, |t| t.elapsed() >= VIP_EVERY) {
                vip_at = Some(Instant::now());
                if let Err(e) = supporters(&paths, &game_dir) {
                    if e != UNCLAIMED {
                        eprintln!("ls-control: supporters: {e}");
                    }
                }
            }
        }
    }
}

/// Someone redeemed an application's invite: the Discord role follows.
pub fn redeemed(paths: &Paths, app: u64, member: &str) {
    let (paths, member) = (Paths { home: paths.home.clone(), id: paths.id.clone() }, member.to_string());
    std::thread::spawn(move || {
        if let Err(e) = call(&paths, "POST", &format!("applications/{app}/redeemed"), Some(&json!({"member": member}))) {
            eprintln!("ls-control: application #{app} redeemed: {e}");
        }
    });
}

/// Someone who came in by an application was removed, or its invite canceled.
pub fn revoked(paths: &Paths, app: u64, by: &str) -> Result<(), String> {
    call(paths, "POST", &format!("applications/{app}/revoke"), Some(&json!({"by": by}))).map(|_| ())
}

/// The applications for this server, ids as text (the app's pages read them that way).
pub fn applications(paths: &Paths) -> Result<Value, String> {
    let v = call(paths, "GET", "applications", None)?;
    let list: Vec<Value> = v["applications"].as_array().into_iter().flatten().map(|a| {
        let mut a = a.clone();
        if let Some(n) = a["id"].as_u64() {
            a["id"] = json!(n.to_string());
        }
        a
    }).collect();
    Ok(json!({"applications": list}))
}

/// One application, as the list service has it (by its number).
pub fn application(paths: &Paths, aid: u64) -> Result<Value, String> {
    let v = call(paths, "GET", "applications", None)?;
    v["applications"].as_array().into_iter().flatten().find(|a| a["id"].as_u64() == Some(aid)).cloned().ok_or_else(|| "That application isn't for this server any more.".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn curl_gets_everything_on_stdin_quoted() {
        let c = curl_config("https://list.lonerstrike.com/roles/link/code", "POST", "lsr1_abc", Some(&json!({"name": "Zo\"ë\\ \n"})));
        assert!(c.contains("header = \"Authorization: Bearer lsr1_abc\"\n"));
        assert!(c.contains(r#"data = "{\"name\":\"Zo\\\"ë\\\\ \\n\"}""#), "{c}");
        assert!(!c.contains("\ndata = \"{\"name"), "quotes escaped");
        assert_eq!(c.lines().filter(|l| l.starts_with("url")).count(), 1);
    }

    #[test]
    fn link_lines_and_supporters() {
        assert_eq!(link_request("1800000000 12 STEAM_0:1:123 Zoë", 1_800_000_005), Some(("Zoë".into(), 12, "STEAM_0:1:123".into())));
        assert_eq!(link_request("1800000000 3 BOT B", 1_800_000_005), None);
        assert_eq!(link_request("1700000000 12 STEAM_0:1:123 Zoë", 1_800_000_005), None);
        let t = with_supporters("\"STEAM_0:0:9\" \"\" \"abc\" \"ce\"\n", &["STEAM_0:1:2".into(), "x\"".into()]);
        assert!(t.contains("\"STEAM_0:1:2\" \"\" \"s\" \"ce\"") && t.contains("STEAM_0:0:9") && !t.contains("x\""));
        assert_eq!(with_supporters(&t, &[]), "\"STEAM_0:0:9\" \"\" \"abc\" \"ce\"\n");
        assert_eq!(status_steamids("# 2 \"a\" 1 STEAM_0:1:5 0 0 active\n# 3 \"b\" 2 BOT"), vec!["STEAM_0:1:5".to_string()]);
        assert_eq!(fail_word("You don't seem to be on this server"), "away");
    }

    #[test]
    fn the_spool_passes_on_only_an_admins_calls() {
        assert!(spool_allows("GET", "applications"));
        assert!(spool_allows("POST", "applications/12/invite") && spool_allows("POST", "applications/12/revoke"));
        for (m, p) in [("GET", "vip"), ("POST", "server"), ("DELETE", "server"), ("POST", "link/code"), ("POST", "applications/x/invite"), ("POST", "applications/1/../server")] {
            assert!(!spool_allows(m, p), "{m} {p}");
        }
        let home = std::env::temp_dir().join(format!("ls-agent-spool-{}", std::process::id()));
        let p = Paths::new(home.clone(), "srv1").unwrap();
        std::fs::create_dir_all(spool_dir(&p)).unwrap();
        // a request that isn't allowed is answered with a refusal (nothing reaches curl)
        std::fs::write(spool_dir(&p).join("abc.req"), json!({"method": "POST", "path": "server", "body": {}}).to_string()).unwrap();
        answer_spool(&p);
        let v: Value = serde_json::from_str(&std::fs::read_to_string(spool_dir(&p).join("abc.resp")).unwrap()).unwrap();
        assert_eq!(v["error"], "That can't be asked from here.");
        assert!(!spool_dir(&p).join("abc.req").exists());
        std::fs::remove_dir_all(home).unwrap();
    }

    #[test]
    fn the_token_file() {
        let home = std::env::temp_dir().join(format!("ls-agent-roles-{}", std::process::id()));
        let p = Paths::new(home.clone(), "srv1").unwrap();
        std::fs::create_dir_all(p.control_dir()).unwrap();
        assert!(token(&p).is_none());
        std::fs::write(p.control_dir().join("roles-token"), "lsr1_abc-_9\n").unwrap();
        assert_eq!(token(&p).as_deref(), Some("lsr1_abc-_9"));
        std::fs::write(p.control_dir().join("roles-token"), "lsr1_a\nb").unwrap();
        assert!(token(&p).is_none());
        assert_eq!(call(&Paths::new(home.clone(), "nope").unwrap(), "GET", "server", None).unwrap_err(), UNCLAIMED);
        std::fs::remove_dir_all(home).unwrap();
    }
}
