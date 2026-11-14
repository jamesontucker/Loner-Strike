//! Community roles, the host's side (docs/notes/Roles.md, docs/ROLES.md; the list service's
//! services/serverlist/roles.py does the rest).
//!
//! - **Claim:** the app asks the list service for a nonce, sets it as the public cvar `ls_claim`
//!   on the running server (gm_link.amxx registers it), and the service reads it back over A2S. The
//!   server token it answers with is kept in `<app dir>/secrets/roles-<server>.token` (0600), and
//!   on a Linux server also beside its ls-control (`roles-token`).
//! - **The /link relay:** a server on this computer that's claimed has its log watched for the line
//!   gm_link writes when a player says /link (`"Name<uid><STEAMID><team>" triggered "ls_link"`):
//!   the service makes the code and the plugin shows it (`ls_link_show <uid> <code>`), or a short
//!   reason (`ls_link_fail <uid> <why>`). Only the uid and SteamID of that line are used, and the
//!   name (which the service checks against the server's own player list).
//! - **Supporters:** with "Honor Loner-Strike supporters" on (or an official server), every few
//!   minutes the SteamIDs on the server are checked with the service and the supporters go in
//!   their own block of users.ini (flag "s", admins.rs).
//! - **Moderator applications:** the owner, or a shared-control Admin of that server, makes a
//!   Moderator invite bound to the applicant's SteamID (control::invite_for_application), which
//!   the service's bot DMs; redeeming it and removing them are told to the service (the Discord
//!   role follows).
//!
//! Every slow call runs off the async threads. Nothing here goes to agents without asking
//! (mcp/routes.rs), and helpers only reach the applications of their own server.

use crate::control::{Helper, Role};
use crate::panel_api::from_panel;
use crate::server::{OnComputer, Shared};
use axum::extract::Path as UrlPath;
use axum::http::{HeaderMap, StatusCode};
use axum::routing::{get, post};
use axum::{Extension, Json, Router};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{Read, Seek, SeekFrom};
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant};

const UNCLAIMED: &str = "This server's claim ended (it was unclaimed, or claimed again). Claim it again to use community roles.";
/// how often the supporters on a server are checked
const VIP_EVERY: Duration = Duration::from_secs(300);

type Reply = (StatusCode, Json<Value>);

fn answer(r: Result<Value, String>) -> Reply {
    match r {
        Ok(v) => (StatusCode::OK, Json(v)),
        Err(e) => (StatusCode::OK, Json(json!({"error": e}))),
    }
}

fn forbidden() -> Reply {
    (StatusCode::FORBIDDEN, Json(json!({"error": "forbidden"})))
}

async fn blocking(f: impl FnOnce() -> Result<Value, String> + Send + 'static) -> Reply {
    answer(tokio::task::spawn_blocking(f).await.unwrap_or_else(|_| Err("Loner-Strike hit an internal error. Try again.".into())))
}

// ---- the server token -------------------------------------------------------------------

fn safe_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

fn token_path(id: &str) -> PathBuf {
    crate::sys::app_dir().join("secrets").join(format!("roles-{id}.token"))
}

fn save_token(id: &str, token: &str) -> Result<(), String> {
    let p = token_path(id);
    if let Some(d) = p.parent() {
        std::fs::create_dir_all(d).map_err(|e| format!("Couldn't save the claim ({e})."))?;
    }
    std::fs::write(&p, token).map_err(|e| format!("Couldn't save the claim ({e})."))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

fn read_token(id: &str) -> Option<String> {
    if !safe_id(id) {
        return None;
    }
    std::fs::read_to_string(token_path(id)).ok().map(|t| t.trim().to_string()).filter(|t| t.starts_with("lsr1_"))
}

/// Forgets a server's claim on this computer (the server was deleted, or its claim ended).
pub fn forget(id: &str) {
    if safe_id(id) {
        let _ = std::fs::remove_file(token_path(id));
    }
}

// ---- the list service -------------------------------------------------------------------

#[cfg(not(test))]
fn base() -> String {
    std::env::var("LS_ROLES_URL").ok().filter(|u| u.starts_with("http://127.0.0.1:")).unwrap_or_else(|| crate::browser::LIST_URL.to_string())
}

#[cfg(test)]
fn base() -> String {
    std::env::var("LS_ROLES_URL").unwrap_or_else(|_| "http://127.0.0.1:9".into())
}

/// One call to the list service's /roles/…: its JSON answer, or its words when it says no.
/// Err(UNCLAIMED) when the token isn't one it knows (the token is forgotten then).
fn call(server: Option<&str>, method: &str, path: &str, body: Option<&Value>) -> Result<Value, String> {
    let token = match server {
        Some(id) => Some(read_token(id).ok_or(UNCLAIMED)?),
        None => None,
    };
    let url = format!("{}/roles/{path}", base());
    let c = crate::sys::http();
    let mut req = match method {
        "GET" => c.get(&url),
        "DELETE" => c.delete(&url),
        _ => c.post(&url),
    }
    .timeout(Duration::from_secs(20))
    .header("Content-Type", "application/json");
    if let Some(t) = &token {
        req = req.header("Authorization", format!("Bearer {t}"));
    }
    if let Some(b) = body {
        req = req.body(b.to_string());
    } else if method != "GET" {
        req = req.body("{}");
    }
    let r = req.send().map_err(|_| "Couldn't reach Loner-Strike's list service. Check the internet connection.".to_string())?;
    let status = r.status().as_u16();
    let v: Value = r.text().ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or(Value::Null);
    if status == 401 && server.is_some() {
        if let Some(id) = server {
            forget(id);
        }
        return Err(UNCLAIMED.into());
    }
    if !(200..300).contains(&status) {
        return Err(v["error"].as_str().map(str::to_string).unwrap_or_else(|| format!("The list service said no (status {status}).")));
    }
    Ok(v)
}

// ---- who's asking ------------------------------------------------------------------------

#[derive(Clone)]
enum Who {
    Owner,
    /// a shared-control Admin of this server (their member id)
    Admin(String),
}

impl Who {
    fn by(&self) -> String {
        match self {
            Who::Owner => "owner".into(),
            Who::Admin(id) => id.clone(),
        }
    }
}

fn owner(headers: &HeaderMap, on: &Option<Extension<OnComputer>>) -> bool {
    from_panel(headers) && on.is_some()
}

fn owner_or_admin(id: &str, headers: &HeaderMap, on: &Option<Extension<OnComputer>>, helper: &Option<Extension<Helper>>) -> Option<Who> {
    if owner(headers, on) {
        return Some(Who::Owner);
    }
    match helper {
        Some(Extension(Helper(m))) if m.role == Role::Admin && m.server == id => Some(Who::Admin(m.id.clone())),
        _ => None,
    }
}

fn entry(id: &str) -> Result<crate::servers::Entry, String> {
    if !safe_id(id) {
        return Err("That isn't one of your servers.".into());
    }
    crate::servers::get(id).filter(|e| e.ready).ok_or_else(|| "That server isn't set up.".to_string())
}

// ---- the owner's card --------------------------------------------------------------------

/// Owner codes shown on the card until they're used or run out: (code, expires)
static OWNER_CODES: Mutex<Option<HashMap<String, (String, u64)>>> = Mutex::new(None);

fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn remember_code(id: &str, v: &Value) {
    if let (Some(c), Some(e)) = (v["owner_code"].as_str(), v["expires"].as_u64()) {
        OWNER_CODES.lock().unwrap_or_else(|e| e.into_inner()).get_or_insert_with(HashMap::new).insert(id.to_string(), (c.to_string(), e));
    }
}

fn view(id: &str, v: &Value) -> Value {
    let linked = v["owner_linked"].as_bool().unwrap_or(false);
    let code = OWNER_CODES.lock().unwrap_or_else(|e| e.into_inner()).as_ref().and_then(|m| m.get(id).cloned()).filter(|(_, e)| *e > now() && !linked);
    json!({
        "claimed": true,
        "official": v["official"].as_bool().unwrap_or(false),
        "owner_linked": linked,
        "owner_code": code.as_ref().map(|c| c.0.clone()),
        "owner_code_expires": code.map(|c| c.1),
        "settings": v["settings"],
        "address": v["address"],
    })
}

async fn api_server(UrlPath(id): UrlPath<String>, headers: HeaderMap, on: Option<Extension<OnComputer>>) -> Reply {
    if !owner(&headers, &on) {
        return forbidden();
    }
    blocking(move || {
        entry(&id)?;
        if read_token(&id).is_none() {
            return Ok(json!({"claimed": false}));
        }
        match call(Some(&id), "GET", "server", None) {
            Ok(v) => Ok(view(&id, &v)),
            Err(e) if e == UNCLAIMED => Ok(json!({"claimed": false, "error": e})),
            Err(e) => Err(e),
        }
    })
    .await
}

/// Sets (or clears) the public cvar the list service reads back.
fn set_claim_cvar(e: &crate::servers::Entry, value: &str) -> Result<(), String> {
    let rcon = e.rcon().ok_or("The server's console password isn't there yet. Start the server once.")?;
    let line = if value.is_empty() { "ls_claim \"\"".to_string() } else { format!("ls_claim {value}") };
    rcon.send(&line).map(|_| ()).map_err(|_| "The server didn't answer on its console. Is it running?".to_string())
}

fn claim(id: &str) -> Result<Value, String> {
    let e = entry(id)?;
    if !crate::servers::running(id) {
        return Err("Start the server first: the list service checks it's yours by asking it.".into());
    }
    let start = call(None, "POST", "claim/start", Some(&json!({"address": e.connect()})))?;
    let (claim, nonce) = (start["claim"].as_str().unwrap_or(""), start["nonce"].as_str().unwrap_or(""));
    if !nonce.starts_with("lsc-") || !nonce[4..].chars().all(|c| c.is_ascii_hexdigit()) {
        return Err("The list service sent something unexpected. Try again.".into());
    }
    set_claim_cvar(&e, nonce)?;
    let mut last = String::new();
    let mut done = None;
    for _ in 0..4 {
        std::thread::sleep(Duration::from_millis(1500));
        match call(None, "POST", "claim/finish", Some(&json!({"claim": claim}))) {
            Ok(v) => {
                done = Some(v);
                break;
            }
            Err(err) if err.contains("ls_claim") => last = err,
            Err(err) => {
                let _ = set_claim_cvar(&e, "");
                return Err(err);
            }
        }
    }
    let _ = set_claim_cvar(&e, "");
    let done = done.ok_or(if last.is_empty() { "The list service couldn't check the server.".to_string() } else { format!("{last} (Is Loner-Strike's gm_link plugin on? Repair the server's files.)") })?;
    let token = done["token"].as_str().filter(|t| t.starts_with("lsr1_")).ok_or("The list service sent something unexpected. Try again.")?;
    save_token(id, token)?;
    if e.remote.is_some() {
        if let Err(err) = crate::control::agent::put_roles_token(&e, token) {
            return Ok(json!({"ok": true, "owner_code": done["owner_code"], "owner_code_expires": done["expires"],
                             "warning": format!("Claimed, but the server's shared control couldn't get the claim ({err}): /link and supporters on it wait until it can. Open Shared control again later.")}));
        }
    }
    remember_code(id, &json!({"owner_code": done["owner_code"], "expires": done["expires"]}));
    Ok(json!({"ok": true, "owner_code": done["owner_code"], "owner_code_expires": done["expires"]}))
}

async fn api_claim(UrlPath(id): UrlPath<String>, headers: HeaderMap, on: Option<Extension<OnComputer>>) -> Reply {
    if !owner(&headers, &on) {
        return forbidden();
    }
    blocking(move || claim(&id)).await
}

async fn api_settings(UrlPath(id): UrlPath<String>, headers: HeaderMap, on: Option<Extension<OnComputer>>, Json(req): Json<Value>) -> Reply {
    if !owner(&headers, &on) {
        return forbidden();
    }
    blocking(move || {
        entry(&id)?;
        let mut settings = serde_json::Map::new();
        for k in ["applications", "discord_mods_decide", "honor_supporters"] {
            if let Some(b) = req.get(k).or_else(|| req.get("settings").and_then(|s| s.get(k))).and_then(Value::as_bool) {
                settings.insert(k.into(), Value::Bool(b));
            }
        }
        let v = call(Some(&id), "POST", "server", Some(&json!({"settings": settings})))?;
        Ok(json!({"ok": true, "settings": v["settings"]}))
    })
    .await
}

async fn api_owner_code(UrlPath(id): UrlPath<String>, headers: HeaderMap, on: Option<Extension<OnComputer>>) -> Reply {
    if !owner(&headers, &on) {
        return forbidden();
    }
    blocking(move || {
        entry(&id)?;
        let v = call(Some(&id), "POST", "server/owner-code", None)?;
        remember_code(&id, &v);
        Ok(json!({"ok": true, "owner_code": v["owner_code"], "owner_code_expires": v["expires"]}))
    })
    .await
}

fn unclaim(id: &str) -> Result<Value, String> {
    entry(id)?;
    let v = match call(Some(id), "DELETE", "server", None) {
        Ok(v) => v,
        Err(e) if e == UNCLAIMED => json!({"revoked": []}),
        Err(e) => return Err(e),
    };
    // its moderators lose their app access and in-game role here (the service took the Discord role)
    let mut removed = 0;
    for r in v["revoked"].as_array().into_iter().flatten() {
        let app = r["application"].as_u64().unwrap_or(0);
        if let Some(m) = crate::control::member_of_application(id, app) {
            if crate::control::remove_member(&m.id).is_ok() {
                removed += 1;
            }
        }
        crate::control::cancel_application_invites(id, app);
    }
    forget(id);
    Ok(json!({"ok": true, "removed": removed}))
}

async fn api_unclaim(UrlPath(id): UrlPath<String>, headers: HeaderMap, on: Option<Extension<OnComputer>>) -> Reply {
    if !owner(&headers, &on) {
        return forbidden();
    }
    blocking(move || unclaim(&id)).await
}

/// A claimed server's (official, honor supporters), or None (not claimed, or the service can't say).
pub fn status_of(id: &str) -> Option<(bool, bool)> {
    read_token(id)?;
    let v = call(Some(id), "GET", "server", None).ok()?;
    Some((v["official"].as_bool().unwrap_or(false), v["settings"]["honor_supporters"].as_bool().unwrap_or(false)))
}

/// "Honor Loner-Strike supporters" on a claimed server (the Admins page's switch).
pub fn set_honor(id: &str, on: bool) -> Result<(), String> {
    if read_token(id).is_none() {
        return Err("Claim this server first (Shared control › Loner-Strike roles), then supporters can be honored here.".into());
    }
    call(Some(id), "POST", "server", Some(&json!({"settings": {"honor_supporters": on}}))).map(|_| ())
}

// ---- applications (the owner, or a shared-control Admin) ---------------------------------

fn app_view(a: &Value) -> Value {
    let mut a = a.clone();
    if let Some(n) = a["id"].as_u64() {
        a["id"] = json!(n.to_string());
    }
    a
}

async fn api_applications(UrlPath(id): UrlPath<String>, headers: HeaderMap, on: Option<Extension<OnComputer>>, helper: Option<Extension<Helper>>) -> Reply {
    if owner_or_admin(&id, &headers, &on, &helper).is_none() {
        return forbidden();
    }
    blocking(move || {
        entry(&id)?;
        let v = call(Some(&id), "GET", "applications", None)?;
        Ok(json!({"applications": v["applications"].as_array().into_iter().flatten().map(app_view).collect::<Vec<_>>()}))
    })
    .await
}

fn application(id: &str, aid: u64) -> Result<Value, String> {
    let v = call(Some(id), "GET", "applications", None)?;
    v["applications"].as_array().into_iter().flatten().find(|a| a["id"].as_u64() == Some(aid)).cloned().ok_or_else(|| "That application isn't for this server any more.".to_string())
}

fn invite(id: &str, aid: u64, who: &Who) -> Result<Value, String> {
    let e = entry(id)?;
    let a = application(id, aid)?;
    if a["state"] != "approved" {
        return Err(format!("That application is {}, not waiting for an invite.", a["state"].as_str().unwrap_or("?")));
    }
    let steam = a["steamid"].as_str().unwrap_or("");
    if e.remote.is_some() {
        return Err("Moderator invites for a server on a Linux machine are made by its shared control service: update the server's files, then try again.".into());
    }
    let (_iid, text, expires) = crate::control::invite_for_application(id, steam, &who.by(), aid)?;
    match call(Some(id), "POST", &format!("applications/{aid}/invite"), Some(&json!({"invite": text, "expires": expires, "by": who.by()}))) {
        Ok(v) => {
            let dm = v["dm"].as_bool().unwrap_or(false);
            Ok(if dm { json!({"ok": true, "dm": true}) } else { json!({"ok": true, "dm": false, "invite": text}) })
        }
        Err(err) => {
            // nobody can be told: the invite stops working at once
            crate::control::cancel_application_invites(id, aid);
            Err(err)
        }
    }
}

async fn api_invite(UrlPath((id, aid)): UrlPath<(String, String)>, headers: HeaderMap, on: Option<Extension<OnComputer>>, helper: Option<Extension<Helper>>) -> Reply {
    let Some(who) = owner_or_admin(&id, &headers, &on, &helper) else { return forbidden() };
    let Ok(aid) = aid.parse::<u64>() else { return answer(Err("That isn't an application.".into())) };
    blocking(move || invite(&id, aid, &who)).await
}

fn revoke(id: &str, aid: u64, who: &Who) -> Result<Value, String> {
    entry(id)?;
    match crate::control::member_of_application(id, aid) {
        // removing them tells the service (control::remove_member → revoked)
        Some(m) => {
            crate::control::remove_member(&m.id)?;
        }
        None => {
            crate::control::cancel_application_invites(id, aid);
            call(Some(id), "POST", &format!("applications/{aid}/revoke"), Some(&json!({"by": who.by()})))?;
        }
    }
    Ok(json!({"ok": true}))
}

async fn api_revoke(UrlPath((id, aid)): UrlPath<(String, String)>, headers: HeaderMap, on: Option<Extension<OnComputer>>, helper: Option<Extension<Helper>>) -> Reply {
    let Some(who) = owner_or_admin(&id, &headers, &on, &helper) else { return forbidden() };
    let Ok(aid) = aid.parse::<u64>() else { return answer(Err("That isn't an application.".into())) };
    blocking(move || revoke(&id, aid, &who)).await
}

/// Someone redeemed an application's invite (control/mod.rs): the service gives the Discord role.
pub fn redeemed(server: &str, app: u64, member: &str) {
    let (server, member) = (server.to_string(), member.to_string());
    std::thread::spawn(move || {
        if let Err(e) = call(Some(&server), "POST", &format!("applications/{app}/redeemed"), Some(&json!({"member": member}))) {
            crate::diagnostics::log_error("roles", &format!("telling the list service about application #{app}'s new moderator: {e}"));
        }
    });
}

/// Someone who came in by an application was removed (control::remove_member).
pub fn revoked(server: &str, app: u64) {
    if let Err(e) = call(Some(server), "POST", &format!("applications/{app}/revoke"), Some(&json!({"by": "owner"}))) {
        if e != UNCLAIMED {
            crate::diagnostics::log_error("roles", &format!("telling the list service application #{app} ended: {e}"));
        }
    }
}

pub fn routes() -> Router<Shared> {
    Router::new()
        .route("/api/roles/server/:id", get(api_server))
        .route("/api/roles/server/:id/claim", post(api_claim))
        .route("/api/roles/server/:id/settings", post(api_settings))
        .route("/api/roles/server/:id/owner-code", post(api_owner_code))
        .route("/api/roles/server/:id/unclaim", post(api_unclaim))
        .route("/api/roles/server/:id/applications", get(api_applications))
        .route("/api/roles/server/:id/applications/:aid/invite", post(api_invite))
        .route("/api/roles/server/:id/applications/:aid/revoke", post(api_revoke))
}

// ---- the /link relay and supporters (servers on this computer) ----------------------------

/// A `triggered "ls_link"` line gm_link wrote: (name, uid, SteamID).
pub fn link_request(line: &str) -> Option<(String, u32, String)> {
    let head = line.trim_end().strip_suffix("\" triggered \"ls_link\"")?;
    // `L 11/14/2026 - 10:00:00: "Name<12><STEAM_0:1:2><CT>`
    let start = head.find(": \"")? + 3;
    let who = &head[start..];
    let mut parts = who.rsplitn(4, '<');
    let _team = parts.next()?;
    let steam = parts.next()?.strip_suffix('>')?;
    let uid = parts.next()?.strip_suffix('>')?.parse::<u32>().ok()?;
    let name = parts.next()?;
    let steam = steam.trim().to_uppercase();
    if !crate::admins::valid_steamid(&steam) || name.is_empty() || name.len() > 64 {
        return None;
    }
    Some((name.to_string(), uid, steam))
}

/// What the plugin says when no code came (a word it knows).
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

fn newest_log(dir: &std::path::Path) -> Option<PathBuf> {
    std::fs::read_dir(dir).ok()?.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("log"))).max_by_key(|p| p.metadata().and_then(|m| m.modified()).ok())
}

/// New whole lines of `path` since `offset` (at most 256 KB at a time).
fn read_new(path: &std::path::Path, offset: &mut u64) -> Vec<String> {
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

fn relay(e: &crate::servers::Entry, line: &str) {
    let Some((name, uid, steam)) = link_request(line) else { return };
    let Some(rcon) = e.rcon() else { return };
    match call(Some(&e.id), "POST", "link/code", Some(&json!({"steamid": steam, "name": name}))) {
        Ok(v) => {
            if let Some(code) = v["code"].as_str().filter(|c| c.len() == 9 && c.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')) {
                let _ = rcon.send(&format!("ls_link_show {uid} {code}"));
            }
        }
        Err(err) => {
            let _ = rcon.send(&format!("ls_link_fail {uid} {}", fail_word(&err)));
        }
    }
}

/// The SteamIDs in a `status` answer.
pub fn status_steamids(status: &str) -> Vec<String> {
    let mut out: Vec<String> = status
        .split(|c: char| c.is_whitespace() || c == '"')
        .map(|w| w.trim().to_uppercase())
        .filter(|w| crate::admins::valid_steamid(w))
        .collect();
    out.sort();
    out.dedup();
    out
}

fn supporters(e: &crate::servers::Entry) -> Result<(), String> {
    let v = call(Some(&e.id), "GET", "server", None)?;
    let official = v["official"].as_bool().unwrap_or(false);
    let honor = v["settings"]["honor_supporters"].as_bool().unwrap_or(false);
    let Some(g) = crate::servers::game_for(&e.id, || None) else { return Ok(()) };
    let ids: Vec<String> = if official {
        call(Some(&e.id), "GET", "vip", None)?["steamids"].as_array().into_iter().flatten().filter_map(|s| s.as_str().map(str::to_string)).collect()
    } else if honor {
        let status = e.rcon().ok_or("no console password")?.send("status").map_err(|_| "the server didn't answer".to_string())?;
        let on: Vec<String> = status_steamids(&status).into_iter().take(64).collect();
        if on.is_empty() {
            vec![]
        } else {
            call(Some(&e.id), "POST", "vip/check", Some(&json!({"steamids": on})))?["steamids"].as_array().into_iter().flatten().filter_map(|s| s.as_str().map(str::to_string)).collect()
        }
    } else {
        vec![]
    };
    crate::admins::set_supporters(&g, &ids).map(|_| ())
}

/// Starts the relay and the supporters check (once, at app start).
pub fn start() {
    std::thread::spawn(|| {
        let mut logs: HashMap<String, (PathBuf, u64)> = HashMap::new();
        let mut vip_at: HashMap<String, Instant> = HashMap::new();
        loop {
            std::thread::sleep(Duration::from_secs(2));
            for e in crate::servers::load() {
                if e.remote.is_some() || !e.ready || read_token(&e.id).is_none() || !crate::servers::running(&e.id) {
                    logs.remove(&e.id);
                    continue;
                }
                let Some(path) = newest_log(&e.dir.join(&e.game).join("logs")) else { continue };
                let state = logs.entry(e.id.clone()).or_insert_with(|| {
                    // from now on: a line written before the app looked isn't answered
                    let len = path.metadata().map(|m| m.len()).unwrap_or(0);
                    (path.clone(), len)
                });
                if state.0 != path {
                    *state = (path.clone(), 0);
                }
                for line in read_new(&state.0, &mut state.1) {
                    if line.ends_with("triggered \"ls_link\"") {
                        relay(&e, &line);
                    }
                }
                let due = vip_at.get(&e.id).map_or(true, |t| t.elapsed() >= VIP_EVERY);
                if due {
                    vip_at.insert(e.id.clone(), Instant::now());
                    if let Err(err) = supporters(&e) {
                        if err != UNCLAIMED {
                            crate::diagnostics::log_error("roles", &format!("supporters on {}: {err}", e.name));
                        }
                    }
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn link_lines() {
        let l = r#"L 11/14/2026 - 10:00:01: "Zoë <3<12><STEAM_0:1:123><CT>" triggered "ls_link""#;
        assert_eq!(link_request(l), Some(("Zoë <3".into(), 12, "STEAM_0:1:123".into())));
        assert_eq!(link_request(r#"L 11/14/2026 - 10:00:01: "Bot<3><BOT><TERRORIST>" triggered "ls_link""#), None);
        assert_eq!(link_request(r#"L 11/14/2026 - 10:00:01: "x<3><STEAM_ID_LAN><>" triggered "ls_link""#), None);
        assert_eq!(link_request(r#"L 11/14/2026 - 10:00:01: "x<3><STEAM_0:1:2><>" triggered "ls_other""#), None);
        // a name that tries to look like a whole line: only the end counts
        let sly = r#"L 11/14/2026 - 10:00:01: "a<1><STEAM_0:0:1><CT>" triggered "ls_link" x<7><STEAM_0:0:2><CT>" triggered "ls_link""#;
        assert_eq!(link_request(sly).map(|r| (r.1, r.2)), Some((7, "STEAM_0:0:2".into())));
        assert_eq!(link_request(r#"L 11/14/2026 - 10:00:01: "x<nope><STEAM_0:1:2><CT>" triggered "ls_link""#), None);
    }

    #[test]
    fn fail_words_and_status() {
        assert_eq!(fail_word("You don't seem to be on this server right now"), "away");
        assert_eq!(fail_word("Too many link codes from this server."), "limit");
        assert_eq!(fail_word(UNCLAIMED), "unclaimed");
        assert_eq!(fail_word("anything"), "error");
        let st = "# userid name uniqueid connected ping loss state\n#  2 \"Zoë\" 12 STEAM_0:1:123 01:02 30 0 active\n#  3 \"Bot\" 13 BOT 00:10 0 0 active\n# 4 \"x\" 14 STEAM_1:0:7 1:00 1 0 active";
        assert_eq!(status_steamids(st), vec!["STEAM_0:1:123".to_string(), "STEAM_1:0:7".to_string()]);
    }

    #[test]
    fn new_lines_only_whole_ones() {
        let d = std::env::temp_dir().join(format!("ls-roles-log-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let p = d.join("L1114000.log");
        std::fs::write(&p, "one\ntwo\nhalf").unwrap();
        let mut off = 0;
        assert_eq!(read_new(&p, &mut off), vec!["one", "two"]);
        std::fs::write(&p, "one\ntwo\nhalf done\nnext\n").unwrap();
        assert_eq!(read_new(&p, &mut off), vec!["half done", "next"]);
        assert!(read_new(&p, &mut off).is_empty());
        std::fs::write(&p, "new\n").unwrap();
        assert_eq!(read_new(&p, &mut off), vec!["new"], "a shorter file starts over");
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn tokens_stay_on_their_server_and_are_checked() {
        assert!(!safe_id("../x") && !safe_id("") && safe_id("s-1_a"));
        assert!(read_token("../secrets").is_none());
    }
}
