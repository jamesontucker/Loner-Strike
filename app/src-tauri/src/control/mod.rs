//! Shared control: friends who also have Loner-Strike help run one of your servers, as an
//! Admin or a Moderator, from their own app.
//!
//! Your app is the hub: their app talks to a third listener here (besides the app window
//! and phone access), over TLS with a certificate made on this computer. Its fingerprint
//! travels in the invite, so their app knows it's really yours without a certificate
//! authority. Every request then goes through `guard`:
//!
//! 1. their key (one per person, made when they accepted the invite; only its hash is kept)
//! 2. the same Loner-Strike version on both sides
//! 3. `rules::check`: nothing gets through unless their role allows it (deny by default)
//! 4. pinned to the one server they were given: whatever server the request names is
//!    replaced by theirs, and a path naming another server is refused
//! 5. a change that went through goes in the server's activity log
//!
//! Invites work once, for one person, for a day. Removing someone ends their key at once.
//! Kept in control.json in the app's folder; the activity in control-log/<server>.jsonl.
//!
//! A server on a Linux machine works differently (agent.rs): its people, invites and
//! activity are kept on the server by ls-control (app/server-agent), which compiles in the
//! same `rules`, and helpers' apps reach the server itself at its https:// address. This app
//! manages it over SSH; the endpoints below pass a Linux server's requests on to it.

pub mod agent;
pub mod client;
pub mod common;
pub mod rules;

use crate::panel_api::from_panel;
use crate::server::{OnComputer, Shared};
use axum::body::{Body, Bytes};
use axum::extract::{Path as UrlPath, Request, State};
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Extension, Json, Router};
use base64::Engine;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Where friends' apps reach this one unless another port is set (TCP; the game uses UDP)
pub const DEFAULT_PORT: u16 = 27030;
/// Unused invites a server can have at once
const MAX_INVITES: usize = 20;
/// How long an invite works
const INVITE_HOURS: u64 = 24;
/// The invite's prefix: what it is, and the format's version
const INVITE_PREFIX: &str = "LS1.";
/// Both sides must run the same version (sent as X-LS-Version)
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Moderator,
    Admin,
}

impl Role {
    fn parse(s: &str) -> Option<Role> {
        match s {
            "admin" => Some(Role::Admin),
            "moderator" => Some(Role::Moderator),
            _ => None,
        }
    }
    fn name(self) -> &'static str {
        match self {
            Role::Admin => "Admin",
            Role::Moderator => "Moderator",
        }
    }
}

/// Someone who helps run one server.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Member {
    pub id: String,
    pub name: String,
    pub role: Role,
    /// the server they were given
    pub server: String,
    /// sha256 of their key (the key itself only ever goes to their app)
    key: String,
    /// their SteamID, for in-game admin (optional)
    #[serde(default)]
    pub steam: String,
    pub added: String,
    /// "owner", or the member id of the Admin who invited them
    #[serde(default)]
    pub invited_by: String,
    #[serde(default)]
    pub seen: Option<u64>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct Invite {
    /// to cancel it by (older invites have none)
    #[serde(default)]
    id: String,
    /// sha256 of the invite's secret
    code: String,
    role: Role,
    server: String,
    #[serde(default)]
    steam: String,
    /// unix seconds
    expires: u64,
    by: String,
}

#[derive(Serialize, Deserialize, Default)]
struct Store {
    #[serde(default)]
    members: Vec<Member>,
    #[serde(default)]
    invites: Vec<Invite>,
    /// the listener's port (None: DEFAULT_PORT)
    #[serde(default)]
    port: Option<u16>,
    /// where friends reach this computer: addresses and host names put in invites
    #[serde(default)]
    addresses: Vec<String>,
}

/// Set on requests a helper sent (through `guard`), for the few handlers that otherwise
/// only take the app window's (OnComputer). Only ever after their role allowed it.
#[derive(Clone, Debug)]
pub struct Helper(pub Member);

static STORE: Mutex<()> = Mutex::new(());

fn store_path() -> PathBuf {
    crate::sys::app_dir().join("control.json")
}

/// The store as saved: empty when there's none yet, an error when it's there but can't be
/// read (busy, scanned by an antivirus, damaged), so nothing is saved over it.
fn read_store() -> Result<Store, String> {
    match std::fs::read_to_string(store_path()) {
        Ok(t) => {
            crate::sys::tighten(&store_path());
            serde_json::from_str(&t).map_err(|_| "The sharing settings file is damaged.".to_string())
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Store::default()),
        Err(e) => Err(format!("Couldn't read the sharing settings ({e}).")),
    }
}

/// For reading: what's saved, or nothing when it can't be read right now.
fn load() -> Store {
    read_store().unwrap_or_default()
}

fn save(s: &Store) -> Result<(), String> {
    let _ = std::fs::create_dir_all(crate::sys::app_dir());
    crate::sys::write_private(&store_path(), serde_json::to_string_pretty(s).unwrap_or_default().as_bytes()).map_err(|e| format!("Couldn't save the sharing settings ({e})."))
}

/// Changes the store under its lock.
fn edit<T>(f: impl FnOnce(&mut Store) -> Result<T, String>) -> Result<T, String> {
    let _g = STORE.lock().unwrap_or_else(|e| e.into_inner());
    // never an empty store saved over one that couldn't be read
    let mut s = read_store()?;
    let out = f(&mut s)?;
    save(&s)?;
    Ok(out)
}

fn now() -> u64 {
    crate::sys::now() as u64
}

fn random_hex(bytes: usize) -> String {
    let mut b = vec![0u8; bytes];
    rand::thread_rng().fill_bytes(&mut b);
    hex(&b)
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn sha(s: &str) -> String {
    hex(&Sha256::digest(s.as_bytes()))
}

pub fn port() -> u16 {
    load().port.unwrap_or(DEFAULT_PORT)
}

/// Whether anything is shared (people or open invites): the listener runs then.
fn in_use(s: &Store) -> bool {
    !s.members.is_empty() || s.invites.iter().any(|i| i.expires > now())
}

// ---- invites ---------------------------------------------------------------------

/// What an invite carries (base64 JSON after INVITE_PREFIX).
#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct InviteText {
    /// addresses (host or host:port) to try, in order
    pub a: Vec<String>,
    /// the listener's port
    pub p: u16,
    /// sha256 of this computer's certificate (empty for a server's own shared control,
    /// agent.rs: its https:// address has a real certificate)
    pub f: String,
    /// the invite's secret
    pub c: String,
    /// the server's name, to show before accepting
    pub s: String,
    /// the role, to show before accepting
    pub r: Role,
}

pub fn encode_invite(t: &InviteText) -> String {
    format!("{INVITE_PREFIX}{}", base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(serde_json::to_vec(t).unwrap_or_default()))
}

pub fn decode_invite(s: &str) -> Option<InviteText> {
    let body = s.trim().strip_prefix(INVITE_PREFIX)?;
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(body).ok()?;
    let t: InviteText = serde_json::from_slice(&bytes).ok()?;
    // no fingerprint only when every address is https:// (checked the usual way)
    let pinned = t.f.len() == 64 || (t.f.is_empty() && t.a.iter().all(|a| a.starts_with("https://")));
    let ok = !t.a.is_empty() && t.a.len() <= 8 && pinned && t.c.len() == 32 && t.s.chars().count() <= 64;
    ok.then_some(t)
}


/// A new invite to `server` (one person, one day).
fn invite(server: &str, role: Role, steam: &str, by: &str, addresses: Option<Vec<String>>) -> Result<(String, String), String> {
    let e = crate::servers::get(server).filter(|e| e.ready).ok_or("That server isn't set up.")?;
    let steam = steam.trim().to_uppercase();
    if !steam.is_empty() && !crate::admins::valid_steamid(&steam) {
        return Err("That isn't a SteamID. It looks like STEAM_0:1:12345.".into());
    }
    let fingerprint = tls::fingerprint()?;
    let secret = random_hex(16);
    let id = random_hex(4);
    let (addrs, port) = edit(|s| {
        if let Some(a) = addresses {
            let a: Vec<String> = a.into_iter().map(|x| x.trim().to_string()).filter(|x| !x.is_empty() && x.len() <= 253).take(8).collect();
            if a.iter().any(|x| !x.starts_with("https://") && (x.matches(':').count() > 1 || x.starts_with('['))) {
                return Err("IPv6 addresses can't be used for sharing yet. Use an IPv4 address, a host name or a tunnel's https:// address.".into());
            }
            s.addresses = a;
        }
        if s.addresses.is_empty() {
            return Err("Say where friends reach this computer first.".into());
        }
        s.invites.retain(|i| i.expires > now());
        if s.invites.iter().filter(|i| i.server == server).count() >= MAX_INVITES {
            return Err(format!("There are already {MAX_INVITES} unused invites for this server. Wait for them to be used or run out (a day)."));
        }
        s.invites.push(Invite { id: id.clone(), code: sha(&secret), role, server: server.into(), steam: steam.clone(), expires: now() + INVITE_HOURS * 3600, by: by.into() });
        Ok((s.addresses.clone(), s.port.unwrap_or(DEFAULT_PORT)))
    })?;
    Ok((id, encode_invite(&InviteText { a: addrs, p: port, f: fingerprint, c: secret, s: e.name, r: role })))
}

/// Turns an invite's secret into a person: their key comes back (once).
fn redeem(secret: &str, name: &str) -> Result<(Member, String), String> {
    let name: String = name.trim().chars().filter(|c| !c.is_control()).take(32).collect();
    let name = if name.is_empty() { "Someone".to_string() } else { name };
    let key = random_hex(32);
    let m = edit(|s| {
        let code = sha(secret.trim());
        let i = s.invites.iter().position(|i| i.code == code && i.expires > now()).ok_or("That invite doesn't work any more. Ask for a new one.")?;
        if crate::servers::get(&s.invites[i].server).is_none() {
            return Err("That server isn't there any more.".into());
        }
        let inv = s.invites.remove(i);
        let m = Member {
            id: random_hex(6),
            name,
            role: inv.role,
            server: inv.server,
            key: sha(&key),
            steam: inv.steam,
            added: crate::sys::stamp(),
            invited_by: inv.by,
            seen: Some(now()),
        };
        s.members.push(m.clone());
        Ok(m)
    })?;
    Ok((m, key))
}

/// A server is deleted: nobody keeps access to its id (a new server could get the same one).
pub fn forget_server(id: &str) {
    // (the file busy for a moment, an antivirus scan: tried again, as a server made later
    // could get the same id)
    for _ in 0..5 {
        let done = edit(|s| {
            s.members.retain(|m| m.server != id);
            s.invites.retain(|i| i.server != id);
            Ok(())
        });
        if done.is_ok() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(400));
    }
    if id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        let _ = std::fs::remove_file(log_dir().join(format!("{id}.jsonl")));
    }
    quiet_if_unused();
}

/// Nothing shared any more: the listener stops and the router's forwarding is taken away.
fn quiet_if_unused() {
    // can't tell right now: left as it is
    if read_store().map_or(true, |s| in_use(&s)) {
        return;
    }
    stop();
    UPNP_GEN.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    if let Some(p) = UPNP.lock().unwrap_or_else(|e| e.into_inner()).take() {
        std::thread::spawn(move || crate::connect::close_tcp(p));
    }
}

/// The app is closing: the router's forwarding goes (its lease would end it within the hour).
pub fn shutdown() {
    if let Some(p) = UPNP.lock().unwrap_or_else(|e| e.into_inner()).take() {
        crate::connect::close_tcp(p);
    }
}

/// Whether a SteamID is someone a Moderator doesn't ban or mute: anyone helping run the
/// server, or its owner, Admins or Moderators in the game (can't tell: as if so).
fn outranks_moderator(server: &str, steamid: &str) -> bool {
    let Ok(s) = read_store() else { return true };
    if s.members.iter().any(|m| m.server == server && !m.steam.is_empty() && m.steam.eq_ignore_ascii_case(steamid)) {
        return true;
    }
    let Some(g) = crate::servers::game_for(server, || None) else { return true };
    crate::admins::has_role(&g, steamid, &[crate::admins::Role::Owner, crate::admins::Role::Admin, crate::admins::Role::Moderator])
}

/// The person with this key.
fn member_for(key: &str) -> Result<Option<Member>, String> {
    if key.len() != 64 {
        return Ok(None);
    }
    let h = sha(key);
    Ok(read_store()?.members.into_iter().find(|m| m.key == h))
}

/// Whether anyone helps run this server (shared control).
pub fn has_helpers(server: &str) -> bool {
    // can't tell right now: as if so (their bans aren't passed on)
    read_store().map_or(true, |s| s.members.iter().any(|m| m.server == server))
}

/// Notes that someone was here (at most once a minute).
fn seen(id: &str) {
    // saved at most once a minute per person, not on every request
    let due = load().members.iter().any(|m| m.id == id && m.seen.map_or(true, |t| now() > t + 60));
    if !due {
        return;
    }
    let _ = edit(|s| {
        if let Some(m) = s.members.iter_mut().find(|m| m.id == id) {
            m.seen = Some(now());
        }
        Ok(())
    });
}

// ---- activity ----------------------------------------------------------------------

fn log_dir() -> PathBuf {
    crate::sys::app_dir().join("control-log")
}

/// A change a helper made goes in the server's log (bodies without anything secret-looking).
fn log_change(m: &Member, path: &str, body: Option<&Value>, undo: Option<Value>) {
    let safe_id = m.server.chars().all(|c| c.is_ascii_alphanumeric() || c == '-');
    if !safe_id {
        return;
    }
    let body = body.map(|b| short(&scrub(b))).unwrap_or(Value::Null);
    let line = json!({"id": random_hex(6), "at": now(), "who": m.name, "member": m.id, "role": m.role, "path": path, "body": body, "undo": undo});
    let _one = LOGGING.lock().unwrap_or_else(|e| e.into_inner());
    let _ = std::fs::create_dir_all(log_dir());
    let file = log_dir().join(format!("{}.jsonl", m.server));
    use std::io::Write;
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&file) {
        let _ = writeln!(f, "{line}");
    }
    // cut back to its newest half once it gets big
    if std::fs::metadata(&file).map(|m| m.len() > LOG_TRIM_AT).unwrap_or(false) {
        if let Ok(t) = std::fs::read_to_string(&file) {
            let mut lines: Vec<&str> = t.lines().collect();
            let mut size: usize = lines.iter().map(|l| l.len() + 1).sum();
            while size > (LOG_TRIM_AT / 2) as usize && lines.len() > 1 {
                size -= lines.remove(0).len() + 1;
            }
            let keep = lines[lines.len().saturating_sub(LOG_KEEP)..].join("
");
            let _ = crate::sys::write_atomic(&file, format!("{keep}
").as_bytes());
        }
    }
}

/// One log writer at a time (an append and a trim)
static LOGGING: Mutex<()> = Mutex::new(());

/// What a change carried, kept short for the log (long texts cut, deep values summed up)
fn short(v: &Value) -> Value {
    match v {
        Value::String(s) if s.chars().count() > 200 => json!(format!("{}…", s.chars().take(200).collect::<String>())),
        Value::Object(o) if o.len() > 40 => json!(format!("{} settings", o.len())),
        Value::Object(o) => Value::Object(o.iter().map(|(k, v)| (k.clone(), short(v))).collect()),
        Value::Array(a) if a.len() > 20 => json!(format!("{} items", a.len())),
        Value::Array(a) => Value::Array(a.iter().map(short).collect()),
        other => other.clone(),
    }
}

/// The activity log is cut back to its newest half past LOG_TRIM_AT bytes
const LOG_TRIM_AT: u64 = 2 * 1024 * 1024;
const LOG_KEEP: usize = 3000;

/// Requests that only read or look something up, though sent as POST: never in the log
fn only_looks(path: &str) -> bool {
    path.ends_with("/console/hints") || path == "/api/control/join"
}

/// A copy of a body without values whose names look secret.
fn scrub(v: &Value) -> Value {
    match v {
        Value::Object(o) => Value::Object(o.iter().map(|(k, v)| (k.clone(), if rules::secret_setting(k) { json!("…") } else { scrub(v) })).collect()),
        Value::Array(a) => Value::Array(a.iter().map(scrub).collect()),
        other => other.clone(),
    }
}

/// The server's log, newest first. Each entry says whether it can be undone (`can_undo`)
/// or already was (`undone`); the undo itself never leaves this computer.
fn activity(server: &str, limit: usize) -> Vec<Value> {
    let Ok(t) = std::fs::read_to_string(log_dir().join(format!("{server}.jsonl"))) else { return vec![] };
    let done = undone();
    let mut v: Vec<Value> = t.lines().filter_map(|l| serde_json::from_str::<Value>(l).ok()).collect();
    let skip = v.len().saturating_sub(limit);
    v.drain(..skip);
    v.reverse();
    for e in v.iter_mut() {
        let key = entry_key(e);
        let can = e["undo"].is_object();
        if let Some(o) = e.as_object_mut() {
            o.remove("undo");
            o.insert("can_undo".into(), json!(can));
            o.insert("undone".into(), json!(done.contains(&key)));
        }
    }
    v
}

/// An entry's name, for undoing it: who and when
fn entry_key(e: &Value) -> String {
    match e["id"].as_str() {
        Some(id) => id.to_string(),
        // (lines from before entries had their own id)
        None => format!("{}:{}", e["member"].as_str().unwrap_or(""), e["at"].as_u64().unwrap_or(0)),
    }
}

/// One undo at a time (undone.json is read, added to and saved)
static UNDOING: Mutex<()> = Mutex::new(());

fn undone_path() -> PathBuf {
    log_dir().join("undone.json")
}

fn undone() -> Vec<String> {
    std::fs::read_to_string(undone_path()).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

/// Puts a change back (the owner, from the activity log): the undo request goes through
/// this app's own window listener, like the owner doing it by hand.
fn undo(st: &Shared, server: &str, key: &str) -> Result<(), String> {
    let _one = UNDOING.lock().unwrap_or_else(|e| e.into_inner());
    let t = std::fs::read_to_string(log_dir().join(format!("{server}.jsonl"))).map_err(|_| "There's nothing to undo.")?;
    let e = t.lines().filter_map(|l| serde_json::from_str::<Value>(l).ok()).rev().find(|e| entry_key(e) == key).ok_or("That change isn't in the log any more.")?;
    if undone().iter().any(|k| k == key) {
        return Err("That's already undone.".into());
    }
    let path = e["undo"]["path"].as_str().ok_or("That change can't be undone.")?.to_string();
    let body = e["undo"]["body"].clone();
    let host = st.hosts.read().unwrap().first().cloned().ok_or("Loner-Strike isn't listening.")?;
    let r: Value = crate::sys::http()
        .post(format!("http://{host}{path}"))
        .header("host", &host)
        .header("x-panel", "1")
        .header("x-target", server)
        .timeout(Duration::from_secs(120))
        .json(&body)
        .send()
        .and_then(|r| r.json())
        .map_err(|e| format!("Couldn't undo it ({e})."))?;
    if let Some(err) = r["error"].as_str() {
        return Err(err.to_string());
    }
    let mut done = undone();
    done.push(key.to_string());
    let _ = std::fs::create_dir_all(log_dir());
    crate::sys::write_atomic(&undone_path(), serde_json::to_string(&done).unwrap_or_default().as_bytes()).map_err(|e| e.to_string())
}

// ---- the listener's guard ------------------------------------------------------------

/// The address a request came from (set per connection by the listener)
#[derive(Clone, Copy)]
pub struct Peer(pub SocketAddr);

/// Tries at accepting invites, per address: (count, window start)
static TRIES: Mutex<Option<HashMap<std::net::IpAddr, (u32, u64)>>> = Mutex::new(None);
const TRIES_MAX: u32 = 10;
const TRIES_WINDOW: u64 = 600;

fn too_many_tries(ip: std::net::IpAddr) -> bool {
    let mut t = TRIES.lock().unwrap_or_else(|e| e.into_inner());
    let t = t.get_or_insert_with(HashMap::new);
    t.retain(|_, (_, start)| now() <= *start + TRIES_WINDOW);
    let e = t.entry(ip).or_insert((0, now()));
    if now() > e.1 + TRIES_WINDOW {
        *e = (0, now());
    }
    e.0 += 1;
    e.0 > TRIES_MAX
}

const NOT_SET_UP: &str = "That server isn't there any more.";

fn refuse(status: StatusCode, why: &str) -> Response {
    (status, Json(json!({"error": why}))).into_response()
}

/// Everything from the sharing listener goes through here (see the module's notes).
pub async fn guard(State(st): State<Shared>, req: Request, next: Next) -> Response {
    let (mut parts, body) = req.into_parts();
    let path = parts.uri.path().to_string();
    let peer = parts.extensions.get::<Peer>().map(|p| p.0.ip());
    if parts.method == Method::POST && path == "/api/control/redeem" {
        // (through a tunnel every try comes from this computer: the invite's secret is the guard)
        if peer.is_some_and(|ip| !ip.is_loopback() && too_many_tries(ip)) {
            return refuse(StatusCode::TOO_MANY_REQUESTS, "Too many tries. Wait a few minutes.");
        }
        // no server is named from outside: the invite says which
        parts.headers.remove("x-target");
        parts.uri = "/api/control/redeem".parse().unwrap_or(parts.uri);
        return next.run(Request::from_parts(parts, body)).await;
    }
    let key = parts.headers.get("authorization").and_then(|v| v.to_str().ok()).and_then(|v| v.strip_prefix("Bearer ")).unwrap_or("").to_string();
    let m = match tokio::task::spawn_blocking(move || member_for(&key)).await {
        Ok(Ok(Some(m))) => m,
        Ok(Ok(None)) => return refuse(StatusCode::UNAUTHORIZED, "The owner took your access away, or it never worked. Ask them for a new invite."),
        // the file is busy for a moment (an antivirus scan): not a reason to say access is gone
        _ => return refuse(StatusCode::SERVICE_UNAVAILABLE, "The owner's Loner-Strike is busy. Try again in a moment."),
    };
    let theirs = parts.headers.get("x-ls-version").and_then(|v| v.to_str().ok()).unwrap_or("");
    if theirs != VERSION {
        return (StatusCode::UPGRADE_REQUIRED, Json(json!({"error": format!("Both of you need the same Loner-Strike version to share a server. The owner has {VERSION}, you have {}.", if theirs.is_empty() { "an older one" } else { theirs }), "version": VERSION}))).into_response();
    }
    let method = parts.method.as_str().to_string();
    let bytes = match axum::body::to_bytes(body, 2 * 1024 * 1024).await {
        Ok(b) => b,
        Err(_) => return refuse(StatusCode::PAYLOAD_TOO_LARGE, "That's too big."),
    };
    let json_body: Option<Value> = if bytes.is_empty() { None } else { serde_json::from_slice(&bytes).ok() };
    if method == "POST" && !bytes.is_empty() && json_body.is_none() {
        return refuse(StatusCode::BAD_REQUEST, "That isn't something Loner-Strike sends.");
    }
    // mode settings: what the server's own game says it has
    let wants_settings = path == "/api/app/mode-settings" && method == "POST";
    let known: Vec<String> = if wants_settings {
        let server = m.server.clone();
        tokio::task::spawn_blocking(move || {
            let Some(g) = crate::servers::game_for(&server, || None) else { return vec![] };
            let mut known: Vec<String> = g.state()["settings"].as_array().into_iter().flatten().filter_map(|s| s["cvar"].as_str().map(|c| c.to_ascii_lowercase())).collect();
            // and every setting the server's plugins have (another mode's, not running now)
            if let Some(e) = crate::servers::get(&server) {
                if let Some(rcon) = e.rcon() {
                    if let Ok(items) = crate::consolehints::for_server(&server, &rcon, false) {
                        known.extend(items.into_iter().filter(|i| i.k == "cvar").map(|i| i.n.to_ascii_lowercase()));
                    }
                }
            }
            known
        })
        .await
        .unwrap_or_default()
    } else {
        vec![]
    };
    let mut verdict = rules::check(&rules::Ask { role: m.role, method: &method, path: &path, server: &m.server, body: json_body.as_ref() }, &|_, cvar| known.iter().any(|k| k == cvar));
    // a Moderator lifting a mute they gave themselves
    if m.role == Role::Moderator && method == "POST" && path == "/api/community/ungag" {
        let who = json_body.as_ref().and_then(|b| b["id"].as_str()).unwrap_or("").to_string();
        let (server, mid) = (m.server.clone(), m.id.clone());
        let own = tokio::task::spawn_blocking(move || {
            let log = std::fs::read_to_string(log_dir().join(format!("{server}.jsonl"))).unwrap_or_default();
            common::gave_last_gag(&log, &mid, &who)
        })
        .await
        .unwrap_or(false);
        if own {
            verdict = rules::Verdict::Allow(None);
        }
    }
    let body = match verdict {
        rules::Verdict::Deny(why) => return refuse(StatusCode::FORBIDDEN, &why),
        rules::Verdict::Allow(Some(v)) => Bytes::from(serde_json::to_vec(&v).unwrap_or_default()),
        rules::Verdict::Allow(None) => bytes,
    };
    if m.role == Role::Moderator && (path == "/api/admins/ban" || path == "/api/community/gag") {
        let who = json_body.as_ref().and_then(|b| b["id"].as_str()).unwrap_or("").trim().to_uppercase();
        let server = m.server.clone();
        let above = tokio::task::spawn_blocking(move || outranks_moderator(&server, &who)).await.unwrap_or(true);
        if above {
            return refuse(StatusCode::FORBIDDEN, "Moderators can't ban or mute the server's owner, its Admins or Moderators, or anyone helping run it.");
        }
    }
    // their server, whatever the request said; their key and anything else of theirs stays here
    let h = &mut parts.headers;
    h.remove("authorization");
    h.remove("cookie");
    h.remove("x-target");
    let Ok(target) = HeaderValue::from_str(&m.server) else { return refuse(StatusCode::FORBIDDEN, NOT_SET_UP) };
    h.insert("x-target", target);
    h.insert("x-panel", HeaderValue::from_static("1"));
    h.insert(axum::http::header::CONTENT_LENGTH, HeaderValue::from(body.len()));
    parts.extensions.insert(Helper(m.clone()));
    let logged: Option<Value> = if method == "POST" { serde_json::from_slice(&body).ok() } else { None };
    // what undoes it, worked out before it happens (the old values)
    let undo = if method == "POST" {
        let (server, p, b) = (m.server.clone(), path.clone(), logged.clone());
        tokio::task::spawn_blocking(move || undo_for(&server, &p, b.as_ref())).await.ok().flatten()
    } else {
        None
    };
    let res = next.run(Request::from_parts(parts, Body::from(body))).await;
    let id = m.id.clone();
    if method != "POST" {
        tokio::task::spawn_blocking(move || seen(&id));
        return res;
    }
    // a change counts once it went through: some answers say an error with 200
    let (rparts, rbody) = res.into_parts();
    let Ok(rbytes) = axum::body::to_bytes(rbody, 16 * 1024 * 1024).await else {
        return refuse(StatusCode::BAD_GATEWAY, "That answer is too big to send.");
    };
    let ok = rparts.status.is_success() && serde_json::from_slice::<Value>(&rbytes).map_or(true, |v| v.get("error").map_or(true, |e| e.is_null()));
    tokio::task::spawn_blocking(move || {
        seen(&id);
        if ok && !only_looks(&path) {
            log_change(&m, &path, logged.as_ref(), undo);
            notify_owner(&st, &m, &path, logged.as_ref());
        }
    });
    Response::from_parts(rparts, Body::from(rbytes))
}

/// What puts a change back: the request that undoes it, with what was there before
/// (settings), or the opposite action (a ban lifted, a mute taken off, a download removed).
/// None: it can't be undone (a restart, a console command, a removal).
fn undo_for(server: &str, path: &str, body: Option<&Value>) -> Option<Value> {
    let b = body?;
    if path == format!("/api/servers/{server}") {
        // the settings as they were, for the ones being changed
        let full = crate::servers::view(&crate::servers::get(server)?);
        let old: serde_json::Map<String, Value> = b.as_object()?.keys().filter_map(|k| full.get(k).map(|v| (k.clone(), v.clone()))).collect();
        return (!old.is_empty()).then(|| json!({"path": path, "body": old}));
    }
    match path {
        "/api/cmd" if b["cmd"] == "gm_set" => {
            let cvar = b["args"][0].as_str()?.to_string();
            let g = crate::servers::game_for(server, || None)?;
            let st = g.state();
            let was = st["settings"].as_array()?.iter().find(|s| s["cvar"].as_str() == Some(cvar.as_str()))?["value"].as_str()?.to_string();
            Some(json!({"path": "/api/cmd", "body": {"cmd": "gm_set", "args": [cvar, was]}}))
        }
        "/api/admins/ban" => Some(json!({"path": "/api/admins/unban", "body": {"kind": "id", "value": b["id"].as_str().unwrap_or("").trim().to_uppercase()}})),
        "/api/community/gag" => Some(json!({"path": "/api/community/ungag", "body": {"id": b["id"]}})),
        "/api/gb/install" => Some(json!({"path": "/api/gb/uninstall", "body": {"mod": b["mod"], "section": b.get("section").cloned().unwrap_or(json!("maps"))}})),
        _ => None,
    }
}

/// Big changes show on the owner's computer: downloads added or removed, the server
/// started, stopped or restarted (the rest is in the activity log).
fn notify_owner(st: &Shared, m: &Member, path: &str, body: Option<&Value>) {
    use tauri_plugin_notification::NotificationExt;
    let server = crate::servers::get(&m.server).map(|e| e.name).unwrap_or_else(|| m.server.clone());
    let what = match path.rsplit('/').next().unwrap_or("") {
        _ if path == "/api/gb/install" => "installed a download on",
        _ if path == "/api/gb/uninstall" => "removed a download from",
        "start" if path.starts_with("/api/servers/") => "started",
        "stop" if path.starts_with("/api/servers/") => "stopped",
        "restart" if path.starts_with("/api/servers/") => "restarted",
        _ => return,
    };
    let _ = body;
    // turned off for this server (its Shared control tab)
    if !crate::servers::get(&m.server).is_some_and(|e| crate::servers::notifies_helpers(&e)) {
        return;
    }
    if let Some(app) = st.app.get() {
        let _ = app.notification().builder().title("Loner-Strike").body(format!("{} ({}) {what} {server}", m.name, m.role.name())).show();
    }
}

// ---- a Linux server's own shared control (agent.rs) -----------------------------------------

/// The server, when its shared control runs on its own machine (a Linux server, set up).
fn on_server(id: &str) -> Option<crate::servers::Entry> {
    crate::servers::get(id).filter(|e| e.ready && agent::used_for(e))
}

/// Whether someone is in this app's own list (they came in the direct way, before their
/// server's own shared control: they keep working through this app until removed).
fn is_local_member(mid: &str) -> bool {
    load().members.iter().any(|m| m.id == mid)
}

/// The owner's tab for a Linux server: people, invites and activity from the server (over
/// SSH), anyone still in this app's own list for it, and whether helpers can reach it.
fn server_view(st: &Shared, e: &crate::servers::Entry) -> Result<Value, String> {
    // set up (or brought up to this version) first; if that can't be done, what's there is shown
    let setup_error = crate::setup_api::resources_for(st).and_then(|res| agent::ensure(e, &res)).err();
    let (mut members, mut invites, mut activity, list_error) = match agent::cli(e, &["list", "--log", "200"]) {
        Ok(v) => (v["members"].as_array().cloned().unwrap_or_default(), v["invites"].as_array().cloned().unwrap_or_default(), v["activity"].as_array().cloned().unwrap_or_default(), None),
        Err(why) => (vec![], vec![], vec![], Some(why)),
    };
    if list_error.is_none() {
        server_game_admins(e, &members, false);
    }
    let s = load();
    for m in s.members.iter().filter(|m| m.server == e.id) {
        let mut v = member_view(m);
        v["via_app"] = json!(true);
        members.push(v);
    }
    invites.extend(s.invites.iter().filter(|i| i.server == e.id && i.expires > now()).map(|i| json!({"id": i.id, "role": i.role, "steam": i.steam, "expires": i.expires, "by": "you", "via_app": true})));
    activity.extend(self::activity(&e.id, 200));
    activity.sort_by_key(|a| std::cmp::Reverse(a["at"].as_u64().unwrap_or(0)));
    activity.truncate(200);
    let url = agent::url(e);
    let reach = url.as_deref().map(agent::reachable);
    let reachable = matches!(&reach, Some(Ok(v)) if v == VERSION);
    let reach_error = match &reach {
        Some(Ok(v)) if v != VERSION => Some(format!("it runs version {v}")),
        Some(Err(why)) => Some(why.clone()),
        _ => None,
    };
    Ok(json!({
        "mode": "server", "members": members, "invites": invites, "activity": activity,
        "url": url, "port": agent::port(e), "domain": e.web_status.domain, "tunnel": e.web_status.mode == "tunnel",
        "reachable": reachable, "reach_error": reach_error,
        "error": list_error.or(setup_error),
    }))
}

/// An invite made by the server's own shared control (it carries the server's https://
/// address). Brought up to date first.
fn server_invite(st: &Shared, e: &crate::servers::Entry, role: Role, steam: &str) -> Result<Value, String> {
    let steam = steam.trim().to_uppercase();
    if !steam.is_empty() && !crate::admins::valid_steamid(&steam) {
        return Err("That isn't a SteamID. It looks like STEAM_0:1:12345.".into());
    }
    if agent::url(e).is_none() {
        return Err("Helpers reach a Linux server at its website status address. Set up Website status for it first (Overview).".into());
    }
    agent::ensure(e, &crate::setup_api::resources_for(st)?)?;
    let mut args = vec!["invite", "--role", if role == Role::Admin { "admin" } else { "moderator" }];
    if !steam.is_empty() {
        args.extend(["--steam", steam.as_str()]);
    }
    let v = agent::cli(e, &args)?;
    let text = v["invite"].as_str().filter(|t| decode_invite(t).is_some()).ok_or("Shared control on the server made an invite that doesn't work. Try again.")?;
    Ok(json!({"ok": true, "invite": text, "id": v["id"]}))
}

/// People on a Linux server's own list with a SteamID are in-game admins there with their
/// role (`gone`: no longer). Kept in this app's copy of the server's admins and sent up.
fn server_game_admins(e: &crate::servers::Entry, people: &[Value], gone: bool) {
    let Some(g) = crate::servers::game_for(&e.id, || None) else { return };
    let mut changed = false;
    for p in people {
        let steam = p["steam"].as_str().unwrap_or("").trim().to_uppercase();
        if !crate::admins::valid_steamid(&steam) {
            continue;
        }
        let role = match p["role"].as_str() {
            Some("admin") => crate::admins::Role::Admin,
            _ => crate::admins::Role::Moderator,
        };
        if gone {
            changed |= crate::admins::set_helper(&g, &steam, "", None).is_ok();
        } else if !crate::admins::has_role(&g, &steam, &[role]) {
            let _ = crate::admins::set_helper(&g, &steam, p["name"].as_str().unwrap_or(""), Some(role));
            changed |= crate::admins::has_role(&g, &steam, &[role]);
        }
    }
    if changed {
        crate::servers::sync_settings(&e.id);
    }
}

// ---- endpoints -------------------------------------------------------------------------

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

fn member_view(m: &Member) -> Value {
    json!({"id": m.id, "name": m.name, "role": m.role, "steam": m.steam, "added": m.added, "seen": m.seen, "invited_by": m.invited_by})
}

/// The owner: who helps run a server, open invites, and where friends reach this computer.
/// A read (the page's getJson sends no X-Panel, like the app's other reads): only names,
/// roles and what changed, never a key; the app window only, like everything here.
async fn api_server(State(st): State<Shared>, UrlPath(id): UrlPath<String>, on: Option<Extension<OnComputer>>) -> Reply {
    if on.is_none() {
        return forbidden();
    }
    blocking(move || {
        if let Some(e) = on_server(&id) {
            return server_view(&st, &e);
        }
        let s = load();
        let members: Vec<Value> = s.members.iter().filter(|m| m.server == id).map(member_view).collect();
        let by = |b: &str| if b == "owner" { "you".to_string() } else { s.members.iter().find(|m| m.id == b).map(|m| m.name.clone()).unwrap_or_else(|| "an Admin".into()) };
        let invites: Vec<Value> = s.invites.iter().filter(|i| i.server == id && i.expires > now()).map(|i| json!({"id": i.id, "role": i.role, "steam": i.steam, "expires": i.expires, "by": by(&i.by)})).collect();
        Ok(json!({"members": members, "invites": invites, "addresses": s.addresses, "port": s.port.unwrap_or(DEFAULT_PORT), "listening": listening(), "activity": activity(&id, 200)}))
    })
    .await
}

#[derive(Deserialize)]
struct InviteReq {
    role: String,
    #[serde(default)]
    steam: String,
    /// where friends reach this computer (kept for later invites)
    #[serde(default)]
    addresses: Option<Vec<String>>,
}

/// The owner makes an invite.
async fn api_invite(State(st): State<Shared>, UrlPath(id): UrlPath<String>, headers: HeaderMap, on: Option<Extension<OnComputer>>, Json(req): Json<InviteReq>) -> Reply {
    if !from_panel(&headers) || on.is_none() {
        return forbidden();
    }
    let Some(role) = Role::parse(&req.role) else { return answer(Err("Pick Admin or Moderator.".into())) };
    // a Linux server: the invite is the server's own (its shared control service makes it)
    if let Some(e) = on_server(&id) {
        return blocking(move || server_invite(&st, &e, role, &req.steam)).await;
    }
    let r = blocking(move || invite(&id, role, &req.steam, "owner", req.addresses).map(|(iid, text)| json!({"ok": true, "invite": text, "id": iid}))).await;
    listen(&st);
    r
}

#[derive(Deserialize)]
struct MemberReq {
    role: String,
    /// the server they help run: a Linux server's people are kept on it (agent.rs)
    #[serde(default)]
    server: Option<String>,
}

/// Which server a request about someone or an invite is for (the page says; older pages don't).
#[derive(Deserialize, Default)]
struct ServerReq {
    #[serde(default)]
    server: Option<String>,
}

async fn api_member(UrlPath(mid): UrlPath<String>, headers: HeaderMap, on: Option<Extension<OnComputer>>, Json(req): Json<MemberReq>) -> Reply {
    if !from_panel(&headers) || on.is_none() {
        return forbidden();
    }
    let Some(role) = Role::parse(&req.role) else { return answer(Err("Pick Admin or Moderator.".into())) };
    blocking(move || {
        // someone kept on the server itself (not in this app's list)
        if let Some(e) = req.server.as_deref().filter(|_| !is_local_member(&mid)).and_then(on_server) {
            let v = agent::cli(&e, &["role", "--member", &mid, "--role", if role == Role::Admin { "admin" } else { "moderator" }])?;
            server_game_admins(&e, std::slice::from_ref(&v["member"]), false);
            return Ok(json!({"ok": true}));
        }
        edit(|s| {
            let m = s.members.iter_mut().find(|m| m.id == mid).ok_or("They don't have access any more.")?;
            m.role = role;
            Ok(m.clone())
        })
        .map(|m| {
            game_admin(&m, Some(m.role));
            json!({"ok": true})
        })
    })
    .await
}

async fn api_remove(UrlPath(mid): UrlPath<String>, headers: HeaderMap, on: Option<Extension<OnComputer>>, req: Option<Json<ServerReq>>) -> Reply {
    if !from_panel(&headers) || on.is_none() {
        return forbidden();
    }
    let server = req.and_then(|Json(r)| r.server);
    blocking(move || {
        if let Some(e) = server.as_deref().filter(|_| !is_local_member(&mid)).and_then(on_server) {
            let v = agent::cli(&e, &["remove", "--member", &mid])?;
            let gone: Vec<Value> = v["removed"].as_array().cloned().unwrap_or_default();
            server_game_admins(&e, &gone, true);
            return Ok(json!({"ok": true, "removed": gone.len(), "left_bans": false}));
        }
        edit(|s| {
            let i = s.members.iter().position(|m| m.id == mid).ok_or("They don't have access any more.")?;
            // invites they made, and the people who came in through them (an Admin's
            // Moderators), end with them
            s.invites.retain(|inv| inv.by != mid);
            let mut gone = vec![s.members.remove(i)];
            let (theirs, rest): (Vec<Member>, Vec<Member>) = std::mem::take(&mut s.members).into_iter().partition(|m| m.invited_by == mid);
            s.members = rest;
            s.invites.retain(|inv| !theirs.iter().any(|t| t.id == inv.by));
            gone.extend(theirs);
            Ok(gone)
        })
        .map(|gone| {
            for m in &gone {
                game_admin(m, None);
            }
            // nobody helps any more: bans changed there while they did aren't passed on to
            // the owner's other servers (it leaves the shared ban list; ticking it again shares)
            let mut left_bans = false;
            if let Some(server) = gone.first().map(|m| m.server.clone()) {
                if !has_helpers(&server) && crate::lists::memberships(&server).iter().any(|l| l == "bans") {
                    crate::lists::leave(&server, "bans");
                    left_bans = true;
                }
            }
            quiet_if_unused();
            json!({"ok": true, "removed": gone.len(), "left_bans": left_bans})
        })
    })
    .await
}

#[derive(Deserialize)]
struct RedeemReq {
    invite: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    version: String,
}

/// A friend's app accepting an invite (the sharing listener, no key yet).
async fn api_redeem(Json(req): Json<RedeemReq>) -> Reply {
    if req.version != VERSION {
        return answer(Err(format!("Both of you need the same Loner-Strike version to share a server. The owner has {VERSION}.")));
    }
    blocking(move || {
        let (m, key) = redeem(&req.invite, &req.name)?;
        let e = crate::servers::get(&m.server).ok_or("That server isn't there any more.")?;
        let m2 = m.clone();
        std::thread::spawn(move || {
            game_admin(&m2, Some(m2.role));
            log_change(&m2, "/api/control/redeem", Some(&json!({"invited_by": m2.invited_by})), None);
        });
        Ok(json!({"ok": true, "key": key, "member": m.id, "role": m.role, "server": {"id": e.id, "name": e.name, "game": e.game}}))
    })
    .await
}

/// What a helper's app may know about the server (the owner's own view has its join
/// password, folders, listen address and start options: only these keys are passed on).
const HELPER_VIEW: &[&str] = &[
    "name", "game", "port", "visibility", "map", "maxplayers", "ready", "state", "uptime", "error", "loner", "installed", "connect", "bots_when_empty", "bots_when_empty_set", "pending_restart",
];

pub fn helper_view(full: &Value) -> Value {
    let mut out = serde_json::Map::new();
    for k in HELPER_VIEW {
        if let Some(v) = full.get(*k) {
            out.insert((*k).to_string(), v.clone());
        }
    }
    Value::Object(out)
}

/// A helper's app: who they are here and the server they help with.
async fn api_me(helper: Option<Extension<Helper>>) -> Reply {
    let Some(Extension(Helper(m))) = helper else { return forbidden() };
    blocking(move || {
        let e = crate::servers::get(&m.server).ok_or("That server isn't there any more.")?;
        Ok(json!({"role": m.role, "role_name": m.role.name(), "name": m.name, "server": helper_view(&crate::servers::view(&e)), "version": VERSION}))
    })
    .await
}

#[derive(Deserialize)]
struct HelperInviteReq {
    #[serde(default)]
    steam: String,
}

/// An Admin invites a Moderator to the same server.
async fn api_helper_invite(helper: Option<Extension<Helper>>, Json(req): Json<HelperInviteReq>) -> Reply {
    let Some(Extension(Helper(m))) = helper else { return forbidden() };
    if m.role != Role::Admin {
        return forbidden();
    }
    blocking(move || invite(&m.server, Role::Moderator, &req.steam, &m.id, None).map(|(iid, text)| json!({"ok": true, "invite": text, "id": iid}))).await
}

#[derive(Deserialize)]
struct AcceptReq {
    invite: String,
    #[serde(default)]
    name: String,
}

/// You, accepting an invite a friend sent: their server joins your list.
async fn api_accept(headers: HeaderMap, on: Option<Extension<OnComputer>>, Json(req): Json<AcceptReq>) -> Reply {
    if !from_panel(&headers) || on.is_none() {
        return forbidden();
    }
    blocking(move || client::accept(&req.invite, &req.name).map(|s| json!({"ok": true, "id": s.id, "name": s.name, "role": s.role}))).await
}

/// You, taking a shared server off your list.
async fn api_leave(UrlPath(id): UrlPath<String>, headers: HeaderMap, on: Option<Extension<OnComputer>>) -> Reply {
    if !from_panel(&headers) || on.is_none() {
        return forbidden();
    }
    blocking(move || client::leave(&id).map(|_| json!({"ok": true}))).await
}

#[derive(Deserialize)]
struct ConsoleReq {
    #[serde(default)]
    command: String,
}

/// A helper typing in the console: their role's commands only (checked by the guard, and
/// here again before it goes to the game).
async fn api_helper_console(helper: Option<Extension<Helper>>, Json(req): Json<ConsoleReq>) -> Reply {
    let Some(Extension(Helper(m))) = helper else { return forbidden() };
    if let Err(why) = rules::console_line(m.role, &req.command) {
        return answer(Err(why));
    }
    blocking(move || {
        // not the owner, an Admin or another helper (as on the Players page)
        if m.role == Role::Moderator {
            if let Some(id) = rules::mod_target(&req.command) {
                if outranks_moderator(&m.server, &id) {
                    return Err("Moderators can't ban or mute the server's owner, its Admins or Moderators, or anyone helping run it.".to_string());
                }
            }
        }
        let e = crate::servers::get(&m.server).ok_or("That server isn't there any more.")?;
        let rcon = e.rcon().ok_or("The server isn't set up yet.")?;
        rcon.send(req.command.trim()).map(|out| json!({"reply": out})).map_err(|_| "The server didn't answer. Check that it's running.".to_string())
    })
    .await
}

#[derive(Deserialize, Default)]
struct HintsReq {
    #[serde(default)]
    refresh: bool,
    #[serde(default)]
    value: Option<String>,
}

/// A helper's console suggestions: only the commands and settings their role may send.
async fn api_helper_hints(helper: Option<Extension<Helper>>, req: Option<Json<HintsReq>>) -> Reply {
    let Some(Extension(Helper(m))) = helper else { return forbidden() };
    let req = req.map(|Json(r)| r).unwrap_or_default();
    blocking(move || {
        let e = crate::servers::get(&m.server).ok_or("That server isn't there any more.")?;
        let rcon = e.rcon().ok_or("The server isn't set up yet.")?;
        if let Some(name) = req.value {
            let name = name.trim().to_string();
            let v = if rules::console_line(m.role, &name).is_ok() { crate::consolehints::value(&m.server, &rcon, &name) } else { None };
            return Ok(json!({"ok": true, "value": v}));
        }
        // (the full list costs the server dozens of commands: a helper gets the kept one)
        let _ = req.refresh;
        let items = crate::consolehints::for_server(&m.server, &rcon, false)?;
        // "kick" alone, "amx_ban x 60": a ban length that fits a Moderator, so bans are offered
        let fits = |n: &str| rules::console_line(m.role, n).is_ok() || rules::console_line(m.role, &format!("{n} x 60")).is_ok() || rules::console_line(m.role, &format!("{n} 60 x")).is_ok();
        let items: Vec<_> = items.into_iter().filter(|i| fits(&i.n)).collect();
        Ok(json!({"ok": true, "items": items}))
    })
    .await
}

// ---- in-game admin, Join, undo, reaching this computer --------------------------------------

/// Someone with a SteamID is an in-game admin on their server with the matching role
/// (None: no longer). A server on another machine gets the change uploaded.
fn game_admin(m: &Member, role: Option<Role>) {
    if m.steam.is_empty() {
        return;
    }
    // removing one of two people with the same SteamID on that server: the other keeps it
    if role.is_none() && load().members.iter().any(|o| o.id != m.id && o.server == m.server && o.steam.eq_ignore_ascii_case(&m.steam)) {
        return;
    }
    let Some(g) = crate::servers::game_for(&m.server, || None) else { return };
    let r = role.map(|r| match r {
        Role::Admin => crate::admins::Role::Admin,
        Role::Moderator => crate::admins::Role::Moderator,
    });
    if crate::admins::set_helper(&g, &m.steam, &m.name, r).is_ok() && g.remote.is_some() {
        crate::servers::sync_settings(&m.server);
    }
}

/// A helper's app asking how to join: the server's address and join password. Their app
/// joins with them and never shows the password.
async fn api_join_info(helper: Option<Extension<Helper>>) -> Reply {
    let Some(Extension(Helper(m))) = helper else { return forbidden() };
    blocking(move || {
        let e = crate::servers::get(&m.server).ok_or("That server isn't there any more.")?;
        // the address the game is reached at from elsewhere: a Linux server's, or the public
        // address the owner set; otherwise the helper's app uses the one it reached this app at
        let public = (e.remote.is_some() || e.public_address.is_some()).then(|| e.connect());
        Ok(json!({"address": public, "port": e.port, "password": crate::servers::join_password(&e), "game": e.game, "unlisted": e.visibility == "unlisted"}))
    })
    .await
}

#[derive(Deserialize, Default)]
struct JoinReq {
    #[serde(default)]
    restart: bool,
}

/// You, joining a server shared with you: its owner's app says where and with what password.
async fn api_shared_join(State(st): State<Shared>, UrlPath(id): UrlPath<String>, headers: HeaderMap, on: Option<Extension<OnComputer>>, req: Option<Json<JoinReq>>) -> Reply {
    if !from_panel(&headers) || on.is_none() {
        return forbidden();
    }
    let restart = req.map(|Json(r)| r.restart).unwrap_or(false);
    blocking(move || {
        let s = client::get(&id).ok_or("That shared server isn't in your list any more.")?;
        let (status, _, bytes) = client::call(&s, "POST", "/api/control/join", Some("application/json"), Bytes::from_static(b"{}"))?;
        let v: Value = serde_json::from_slice(&bytes).map_err(|_| "The owner's Loner-Strike answered strangely.")?;
        if let Some(e) = v["error"].as_str() {
            return Err(e.to_string());
        }
        if status != 200 {
            return Err("The owner's Loner-Strike refused.".into());
        }
        // the owner's public address for it, else the host this app reaches the owner's
        // app at with the game's port (a tunnel's name carries no game traffic)
        let reached = s.addresses.first().map(|a| a.as_str()).unwrap_or("");
        let fallback = if reached.starts_with("https://") || reached.is_empty() {
            None
        } else {
            Some(format!("{}:{}", reached.split(':').next().unwrap_or(reached), v["port"].as_u64().unwrap_or(27015)))
        };
        let address = v["address"].as_str().filter(|a| !a.is_empty() && a.len() < 300).map(String::from).or(fallback).ok_or("The owner hasn't set a public address for the game. Ask them to set one (Overview, Public address).")?;
        let address = address.as_str();
        // only an address and a plain password reach the game's command line
        if address.starts_with('-') || !address.chars().all(|c| c.is_ascii_alphanumeric() || ".:-".contains(c)) {
            return Err("The owner's Loner-Strike sent an address that isn't one.".into());
        }
        let pw = v["password"].as_str().map(String::from).filter(|p| !p.is_empty());
        if pw.as_deref().is_some_and(|p| p.starts_with('-') || !p.chars().all(|c| c.is_ascii_alphanumeric() || "_-".contains(c))) {
            return Err("The owner's Loner-Strike sent a password the game can't use.".into());
        }
        crate::servers::launch_join(&st, v["game"].as_str().unwrap_or("cstrike"), address, pw, v["unlisted"].as_bool().unwrap_or(false), restart, true)
    })
    .await
}

/// The owner cancels an invite nobody has used yet (sent to the wrong person, or leaked).
async fn api_cancel_invite(UrlPath(iid): UrlPath<String>, headers: HeaderMap, on: Option<Extension<OnComputer>>, req: Option<Json<ServerReq>>) -> Reply {
    if !from_panel(&headers) || on.is_none() {
        return forbidden();
    }
    let server = req.and_then(|Json(r)| r.server);
    blocking(move || {
        let here = load().invites.iter().any(|i| !i.id.is_empty() && i.id == iid);
        if let Some(e) = server.as_deref().filter(|_| !here).and_then(on_server) {
            return agent::cli(&e, &["cancel", "--invite", &iid]).map(|_| json!({"ok": true}));
        }
        let r = edit(|s| {
            let n = s.invites.len();
            s.invites.retain(|i| i.id.is_empty() || i.id != iid);
            if s.invites.len() == n {
                return Err("That invite was already used, cancelled or ran out.".into());
            }
            Ok(json!({"ok": true}))
        });
        quiet_if_unused();
        r
    })
    .await
}

#[derive(Deserialize)]
struct UndoReq {
    key: String,
}

/// The owner puts back a change someone made.
async fn api_undo(State(st): State<Shared>, UrlPath(id): UrlPath<String>, headers: HeaderMap, on: Option<Extension<OnComputer>>, Json(req): Json<UndoReq>) -> Reply {
    if !from_panel(&headers) || on.is_none() {
        return forbidden();
    }
    blocking(move || undo(&st, &id, &req.key).map(|_| json!({"ok": true}))).await
}

/// Keeps the router's port open while asked (renewed before the lease runs out)
static UPNP: Mutex<Option<u16>> = Mutex::new(None);
/// Bumped when the renewing stops (another port): an older renewing thread then ends
static UPNP_GEN: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Renews the router's forwarding before its lease runs out, while anything is shared: once
/// the last invite has run out with nobody having joined (or everyone was removed), the
/// listener stops and the port is closed, instead of staying open until the app quits.
fn keep_open() {
    let gen = UPNP_GEN.load(std::sync::atomic::Ordering::SeqCst);
    std::thread::spawn(move || loop {
        std::thread::sleep(crate::connect::LEASE / 2);
        if UPNP_GEN.load(std::sync::atomic::Ordering::SeqCst) != gen {
            return;
        }
        if read_store().is_ok_and(|s| !in_use(&s)) {
            quiet_if_unused();
            return;
        }
        let Some(p) = *UPNP.lock().unwrap_or_else(|e| e.into_inner()) else { return };
        let _ = crate::connect::open_tcp(p);
    });
}

/// The owner: opens the sharing port on the router (UPnP) and adds the internet address to
/// the ones invites carry. Kept open while the app runs.
async fn api_open_port(headers: HeaderMap, on: Option<Extension<OnComputer>>) -> Reply {
    if !from_panel(&headers) || on.is_none() {
        return forbidden();
    }
    blocking(|| {
        let port = port();
        let (ip, shared) = crate::connect::open_tcp(port)?;
        if shared {
            return Err(format!("Your router's internet address ({ip}) is shared with others by your internet provider (CGNAT), so friends can't reach it from outside. Use a virtual LAN or a tunnel instead."));
        }
        let first = UPNP.lock().unwrap_or_else(|e| e.into_inner()).replace(port).is_none();
        if first {
            keep_open();
        }
        Ok(json!({"ok": true, "address": ip.to_string(), "port": port}))
    })
    .await
}

#[derive(Deserialize)]
struct PortReq {
    port: u16,
}

/// The owner picks another port (invites made after it carry the new one).
async fn api_port(State(st): State<Shared>, headers: HeaderMap, on: Option<Extension<OnComputer>>, Json(req): Json<PortReq>) -> Reply {
    if !from_panel(&headers) || on.is_none() {
        return forbidden();
    }
    if req.port < 1024 || (27015..=27020).contains(&req.port) {
        return answer(Err("Pick a port from 1024 to 65535 that isn't a game port (27015 to 27020).".into()));
    }
    if req.port == port() {
        return answer(Ok(json!({"ok": true, "port": req.port})));
    }
    let r = blocking(move || edit(|s| {
        s.port = Some(req.port);
        Ok(json!({"ok": true, "port": req.port}))
    }))
    .await;
    if r.1 .0.get("error").is_some() {
        return r;
    }
    // the listener moves to it (the old one lets go of its port a moment later), and the
    // router's forwarding with it when it was open
    stop();
    let was_open = UPNP.lock().unwrap_or_else(|e| e.into_inner()).take();
    UPNP_GEN.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let new = req.port;
    std::thread::spawn(move || {
        if let Some(old) = was_open {
            crate::connect::close_tcp(old);
        }
        for _ in 0..20 {
            listen(&st);
            if listening().is_some() {
                break;
            }
            std::thread::sleep(Duration::from_millis(150));
        }
        if was_open.is_some() && crate::connect::open_tcp(new).is_ok() {
            *UPNP.lock().unwrap_or_else(|e| e.into_inner()) = Some(new);
            keep_open();
        }
    });
    r
}

/// You: your Steam name, to fill in when you accept an invite.
async fn api_whoami(on: Option<Extension<OnComputer>>) -> Reply {
    if on.is_none() {
        return forbidden();
    }
    blocking(|| Ok(json!({"name": crate::steam::persona_name()}))).await
}

/// The app window's endpoints: the owner's, and yours for servers shared with you.
pub fn local_routes() -> Router<Shared> {
    Router::new()
        .route("/api/control/accept", post(api_accept))
        .route("/api/control/whoami", get(api_whoami))
        .route("/api/control/shared/:id/join", post(api_shared_join))
        .route("/api/control/server/:id/undo", post(api_undo))
        .route("/api/control/invite/:iid/cancel", post(api_cancel_invite))
        .route("/api/control/open-port", post(api_open_port))
        .route("/api/control/port", post(api_port))
        .route("/api/control/shared/:id/leave", post(api_leave))
        .route("/api/control/server/:id", get(api_server))
        .route("/api/control/server/:id/invite", post(api_invite))
        .route("/api/control/member/:mid", post(api_member))
        .route("/api/control/member/:mid/remove", post(api_remove))
}

/// The sharing listener's own endpoints (besides the shared ones `rules` lets through).
pub fn share_routes() -> Router<Shared> {
    Router::new()
        .route("/api/control/redeem", post(api_redeem))
        .route("/api/control/me", get(api_me))
        .route("/api/control/invite", post(api_helper_invite))
        .route("/api/control/join", post(api_join_info))
        // the same paths the app window uses for its own servers (those are computer-only)
        .route("/api/servers/:id/console/send", post(api_helper_console))
        .route("/api/servers/:id/console/hints", post(api_helper_hints))
}

// ---- the listener ------------------------------------------------------------------------

static RUNNING: Mutex<Option<(u16, tokio::sync::oneshot::Sender<()>)>> = Mutex::new(None);

fn listening() -> Option<u16> {
    RUNNING.lock().unwrap_or_else(|e| e.into_inner()).as_ref().map(|(p, _)| *p)
}

/// Starts the sharing listener if something is shared and it isn't running (at start-up
/// and after an invite). A port that's busy is left for the owner's page to show.
/// The app has started: each Linux server's shared control service is brought to this app's
/// version (after an update helpers' apps would otherwise be refused until the owner opened
/// the Share control tab). In the background, a moment after start, one server at a time.
pub fn refresh_agents(st: &Shared) {
    let st = st.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(20));
        let Ok(res) = crate::setup_api::resources_for(&st) else { return };
        for e in crate::servers::load() {
            if !e.ready || !agent::used_for(&e) || agent::url(&e).is_none() {
                continue;
            }
            if let Err(why) = agent::ensure(&e, &res) {
                eprintln!("shared control on {}: {why}", e.id);
            }
        }
    });
}

pub fn listen(st: &Shared) {
    if listening().is_some() || !in_use(&load()) {
        return;
    }
    let port = port();
    let config = match tls::server_config() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("shared control: {e}");
            return;
        }
    };
    let Ok(listener) = std::net::TcpListener::bind(("0.0.0.0", port)) else {
        eprintln!("shared control: port {port} is busy");
        return;
    };
    let (tx, rx) = tokio::sync::oneshot::channel::<()>();
    *RUNNING.lock().unwrap_or_else(|e| e.into_inner()) = Some((port, tx));
    let app = crate::server::router(st.clone(), crate::server::Listener::Share);
    tauri::async_runtime::spawn(async move {
        serve(listener, config, app, rx).await;
        // it stopped (or never got going): not running any more, unless a newer one is
        let mut r = RUNNING.lock().unwrap_or_else(|e| e.into_inner());
        if r.as_ref().is_some_and(|(p, tx)| *p == port && tx.is_closed()) {
            *r = None;
        }
    });
}

/// Stops it (sharing turned off).
pub fn stop() {
    if let Some((_, tx)) = RUNNING.lock().unwrap_or_else(|e| e.into_inner()).take() {
        let _ = tx.send(());
    }
}

/// Connections open from one address at once, and how long one may last
static PER_IP: Mutex<Option<HashMap<std::net::IpAddr, usize>>> = Mutex::new(None);
const PER_IP_MAX: usize = 8;
const CONNECTION_FOR: Duration = Duration::from_secs(120);

/// At most this many connections at once
const MAX_CONNECTIONS: usize = 64;

async fn serve(listener: std::net::TcpListener, config: Arc<rustls::ServerConfig>, app: Router, mut stop: tokio::sync::oneshot::Receiver<()>) {
    use hyper_util::rt::{TokioIo, TokioTimer};
    use hyper_util::service::TowerToHyperService;
    listener.set_nonblocking(true).ok();
    let Ok(listener) = tokio::net::TcpListener::from_std(listener) else { return };
    let acceptor = tokio_rustls::TlsAcceptor::from(config);
    let slots = Arc::new(tokio::sync::Semaphore::new(MAX_CONNECTIONS));
    loop {
        let (tcp, peer) = tokio::select! {
            _ = &mut stop => return,
            r = listener.accept() => match r {
                Ok(x) => x,
                // out of connections or handles: a moment's rest, not a busy loop
                Err(_) => {
                    tokio::time::sleep(Duration::from_millis(200)).await;
                    continue;
                }
            },
        };
        let Ok(slot) = slots.clone().try_acquire_owned() else { continue };
        // a few at a time from one address, so one address can't take every slot
        let ip = peer.ip();
        // (a tunnel on this computer brings everyone from 127.0.0.1: only the total counts then)
        if !ip.is_loopback() {
            let mut per = PER_IP.lock().unwrap_or_else(|e| e.into_inner());
            let n = per.get_or_insert_with(HashMap::new).entry(ip).or_insert(0);
            if *n >= PER_IP_MAX {
                continue;
            }
            *n += 1;
        }
        let acceptor = acceptor.clone();
        let app = app.clone();
        tokio::spawn(async move {
            let _slot = slot;
            let served = async {
                let Ok(Ok(tls)) = tokio::time::timeout(Duration::from_secs(10), acceptor.accept(tcp)).await else { return };
                // the address it came from, for the guard (tries at accepting invites)
                let svc = TowerToHyperService::new(app.layer(Extension(Peer(peer))));
                let _ = hyper::server::conn::http1::Builder::new().timer(TokioTimer::new()).header_read_timeout(Duration::from_secs(20)).serve_connection(TokioIo::new(tls), svc).await;
            };
            // no connection kept forever (a slow sender included); the app connects again
            let _ = tokio::time::timeout(CONNECTION_FOR, served).await;
            if let Some(per) = PER_IP.lock().unwrap_or_else(|e| e.into_inner()).as_mut() {
                if let Some(n) = per.get_mut(&ip) {
                    *n = n.saturating_sub(1);
                    if *n == 0 {
                        per.remove(&ip);
                    }
                }
            }
        });
    }
}

/// This computer's certificate for the sharing listener: made once, kept in the app's folder.
pub mod tls {
    use base64::Engine;
    use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
    use sha2::{Digest, Sha256};
    use std::sync::Arc;

    #[derive(serde::Serialize, serde::Deserialize)]
    struct Kept {
        cert: String,
        key: String,
    }

    fn path() -> std::path::PathBuf {
        crate::sys::app_dir().join("control-tls.json")
    }

    static MAKING: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// (certificate DER, private key DER)
    fn pair() -> Result<(Vec<u8>, Vec<u8>), String> {
        let _one = MAKING.lock().unwrap_or_else(|e| e.into_inner());
        let b64 = base64::engine::general_purpose::STANDARD;
        // a new certificate only when there's none: one that can't be read right now isn't
        // replaced (friends' apps know this computer by it, and invites carry it)
        match std::fs::read_to_string(path()) {
            Ok(t) => {
                crate::sys::tighten(&path());
                let k = serde_json::from_str::<Kept>(&t).map_err(|_| "The sharing certificate file is damaged.".to_string())?;
                return match (b64.decode(&k.cert), b64.decode(&k.key)) {
                    (Ok(c), Ok(k)) => Ok((c, k)),
                    _ => Err("The sharing certificate file is damaged.".into()),
                };
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(format!("Couldn't read the sharing certificate ({e}).")),
        }
        let made = rcgen::generate_simple_self_signed(vec!["loner-strike".to_string()]).map_err(|e| format!("Couldn't make the sharing certificate ({e})."))?;
        let (cert, key) = (made.cert.der().to_vec(), made.key_pair.serialize_der());
        let kept = Kept { cert: b64.encode(&cert), key: b64.encode(&key) };
        let _ = std::fs::create_dir_all(crate::sys::app_dir());
        crate::sys::write_private(&path(), serde_json::to_string(&kept).unwrap_or_default().as_bytes()).map_err(|e| format!("Couldn't save the sharing certificate ({e})."))?;
        Ok((cert, key))
    }

    /// sha256 of the certificate, as invites carry it
    pub fn fingerprint() -> Result<String, String> {
        let (cert, _) = pair()?;
        Ok(super::hex(&Sha256::digest(&cert)))
    }

    pub fn server_config() -> Result<Arc<rustls::ServerConfig>, String> {
        let (cert, key) = pair()?;
        let provider = Arc::new(rustls::crypto::ring::default_provider());
        let config = rustls::ServerConfig::builder_with_provider(provider)
            .with_safe_default_protocol_versions()
            .map_err(|e| e.to_string())?
            .with_no_client_auth()
            .with_single_cert(vec![CertificateDer::from(cert)], PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(key)))
            .map_err(|e| format!("The sharing certificate is damaged ({e})."))?;
        Ok(Arc::new(config))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invite_round_trip() {
        let t = InviteText { a: vec!["203.0.113.40".into(), "share.example.com".into()], p: 27030, f: "a".repeat(64), c: "b".repeat(32), s: "Friday Frags".into(), r: Role::Admin };
        let s = encode_invite(&t);
        assert!(s.starts_with("LS1."));
        assert_eq!(decode_invite(&s), Some(t));
        assert_eq!(decode_invite(&format!("  {s}\n")).map(|t| t.p), Some(27030), "pasted with spaces");
        assert!(decode_invite("LS1.nonsense").is_none());
        // a server's own shared control: no fingerprint, https:// only
        let t = InviteText { a: vec!["https://status.example.com/control".into()], p: 443, f: String::new(), c: "b".repeat(32), s: "Friday Frags".into(), r: Role::Moderator };
        assert_eq!(decode_invite(&encode_invite(&t)), Some(t));
        let t = InviteText { a: vec!["203.0.113.40".into()], p: 27030, f: String::new(), c: "b".repeat(32), s: "x".into(), r: Role::Admin };
        assert!(decode_invite(&encode_invite(&t)).is_none(), "a direct address needs the fingerprint");
        // one ls-control (app/server-agent) made, used once in a test container
        let made = decode_invite("LS1.eyJhIjpbImh0dHBzOi8vc3RhdHVzLmV4YW1wbGUuY29tL2NvbnRyb2wiXSwicCI6NDQzLCJmIjoiIiwiYyI6Ijc3MjU5YzYwNjRlYWI3NmEwYWZlYTc0YzkxMjdiZjJjIiwicyI6IkZyaWRheSBGcmFncyIsInIiOiJtb2RlcmF0b3IifQ").unwrap();
        assert_eq!((made.a[0].as_str(), made.r, made.s.as_str()), ("https://status.example.com/control", Role::Moderator, "Friday Frags"));
        assert!(decode_invite("hello").is_none());
    }

    #[test]
    fn steam_ids() {
        let ok = |s: &str| s.is_empty() || crate::admins::valid_steamid(s);
        assert!(ok("") && ok("STEAM_0:1:12345"));
        assert!(!ok("STEAM_0:1:1; rcon_password x") && !ok("bob"));
    }

    #[test]
    fn logs_hide_secrets() {
        let v = scrub(&json!({"name": "x", "password": "hunter2", "inner": {"rcon_password": "y", "ok": 1}}));
        assert_eq!(v, json!({"name": "x", "password": "…", "inner": {"rcon_password": "…", "ok": 1}}));
    }
}
