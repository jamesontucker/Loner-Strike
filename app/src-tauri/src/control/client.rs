//! The other end of shared control: servers friends shared with you, in your own app.
//!
//! Each one is kept in shared-servers.json (in the app's folder) under a local id
//! ("sh-" + 8 hex), with where the owner's app is, its certificate's fingerprint, your
//! key and your role there. Your app then works on it like on one of your own: a request
//! for it (X-Target, or /api/servers/<local id>/…) is passed to the owner's app with your
//! key over TLS pinned to that fingerprint, and the owner's app decides what you may do.

use super::{decode_invite, InviteText, Role, VERSION};
use axum::body::{Body, Bytes};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{DigitallySignedStruct, SignatureScheme};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub const PREFIX: &str = "sh-";
/// The most an owner's app may send back for one request
const MAX_REPLY: u64 = 32 * 1024 * 1024;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Shared {
    /// the id it has in this app
    pub id: String,
    /// its id in the owner's app
    pub remote: String,
    pub name: String,
    pub game: String,
    pub role: Role,
    /// where the owner's app is: "host:port", or "https://host" (a tunnel with a real certificate)
    pub addresses: Vec<String>,
    /// sha256 of the owner's certificate (direct addresses)
    pub fingerprint: String,
    key: String,
    pub added: String,
}

static FILE: Mutex<()> = Mutex::new(());

fn path() -> std::path::PathBuf {
    crate::sys::app_dir().join("shared-servers.json")
}

/// What's saved: none yet is an empty list; a file that can't be read now is an error,
/// so a change never saves over it with only the new entry.
fn read_all() -> Result<Vec<Shared>, String> {
    match std::fs::read_to_string(path()) {
        Ok(t) => serde_json::from_str(&t).map_err(|_| "Your list of shared servers is damaged.".to_string()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(vec![]),
        Err(e) => Err(format!("Couldn't read your list of shared servers ({e}).")),
    }
}

pub fn load() -> Vec<Shared> {
    read_all().unwrap_or_default()
}

fn save(all: &[Shared]) -> Result<(), String> {
    let _ = std::fs::create_dir_all(crate::sys::app_dir());
    crate::sys::write_atomic(&path(), serde_json::to_string_pretty(all).unwrap_or_default().as_bytes()).map_err(|e| format!("Couldn't save the shared server ({e})."))
}

pub fn get(id: &str) -> Option<Shared> {
    load().into_iter().find(|s| s.id == id)
}

/// Whether an id (from X-Target or a path) is a shared server's.
pub fn is_shared(id: &str) -> bool {
    id.strip_prefix(PREFIX).is_some_and(|h| h.len() == 8 && h.chars().all(|c| c.is_ascii_hexdigit()))
}

/// Requests about the server itself, which go to the owner's app when a shared server is
/// open. Everything else (this app's own info, setup, the server list...) stays here, as
/// it does for your own game, whatever server the page has open.
pub fn about_the_server(path: &str) -> bool {
    const PREFIXES: &[&str] = &[
        "/api/servers/", "/api/state", "/api/health", "/api/maps", "/api/modes/", "/thumb/", "/api/cmd", "/api/admins", "/api/community", "/api/extras", "/api/welcome",
        "/api/modevote", "/api/scrim", "/api/servercontent", "/api/gb/", "/api/sounds", "/api/mods/plugins", "/api/app/mode-settings", "/api/mapflag", "/api/control/me", "/api/control/invite",
    ];
    // "/api/servers/" covers a server's own pages, never the list itself; the others are a
    // path and what's under it ("/api/state", not "/api/statex")
    PREFIXES.iter().any(|p| if p.ends_with('/') { path.starts_with(p) } else { path == *p || path.starts_with(&format!("{p}/")) })
}

// ---- TLS: the owner's certificate, by its fingerprint ---------------------------------

/// Accepts exactly the certificate whose sha256 the invite carried (no certificate
/// authority, no host name: the owner's app made it, and only its fingerprint counts).
#[derive(Debug)]
struct Pinned {
    fingerprint: String,
    provider: Arc<rustls::crypto::CryptoProvider>,
}

impl ServerCertVerifier for Pinned {
    fn verify_server_cert(&self, end_entity: &CertificateDer<'_>, _: &[CertificateDer<'_>], _: &ServerName<'_>, _: &[u8], _: UnixTime) -> Result<ServerCertVerified, rustls::Error> {
        if super::hex(&Sha256::digest(end_entity.as_ref())) == self.fingerprint {
            Ok(ServerCertVerified::assertion())
        } else {
            Err(rustls::Error::General("not the owner's Loner-Strike".into()))
        }
    }
    fn verify_tls12_signature(&self, message: &[u8], cert: &CertificateDer<'_>, dss: &DigitallySignedStruct) -> Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls12_signature(message, cert, dss, &self.provider.signature_verification_algorithms)
    }
    fn verify_tls13_signature(&self, message: &[u8], cert: &CertificateDer<'_>, dss: &DigitallySignedStruct) -> Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(message, cert, dss, &self.provider.signature_verification_algorithms)
    }
    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.provider.signature_verification_algorithms.supported_schemes()
    }
}

/// A client that only talks to the app with that certificate (one per fingerprint, kept).
fn client(fingerprint: &str) -> Result<reqwest::blocking::Client, String> {
    static CLIENTS: Mutex<Option<HashMap<String, reqwest::blocking::Client>>> = Mutex::new(None);
    if let Some(c) = CLIENTS.lock().unwrap_or_else(|e| e.into_inner()).as_ref().and_then(|m| m.get(fingerprint)) {
        return Ok(c.clone());
    }
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let config = rustls::ClientConfig::builder_with_provider(provider.clone())
        .with_safe_default_protocol_versions()
        .map_err(|e| e.to_string())?
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(Pinned { fingerprint: fingerprint.to_string(), provider }))
        .with_no_client_auth();
    let c = reqwest::blocking::Client::builder()
        .tls_backend_preconfigured(config)
        .connect_timeout(Duration::from_secs(6))
        .timeout(Duration::from_secs(60))
        .build()
        .map_err(|e| e.to_string())?;
    CLIENTS.lock().unwrap_or_else(|e| e.into_inner()).get_or_insert_with(HashMap::new).insert(fingerprint.to_string(), c.clone());
    Ok(c)
}

fn tunnel_client() -> Result<reqwest::blocking::Client, String> {
    static C: std::sync::OnceLock<reqwest::blocking::Client> = std::sync::OnceLock::new();
    if let Some(c) = C.get() {
        return Ok(c.clone());
    }
    // (the same TLS setup as the app's other HTTPS client)
    crate::sys::http();
    let c = reqwest::blocking::Client::builder().connect_timeout(Duration::from_secs(6)).timeout(Duration::from_secs(60)).build().map_err(|e| e.to_string())?;
    Ok(C.get_or_init(|| c).clone())
}

/// The base URL and client for one of the owner's addresses.
fn endpoint(address: &str, port: u16, fingerprint: &str) -> Result<(String, reqwest::blocking::Client), String> {
    // only a host and maybe a port: an invite is pasted text, and anything more (a path, a
    // query, a user@) would send the request somewhere other than the owner's app
    let bare = address.strip_prefix("https://").unwrap_or(address).trim_end_matches('/');
    // a server's own shared control (agent.rs) is at /control on its https:// address: that
    // one path only
    let (bare, path) = match bare.strip_suffix("/control") {
        Some(h) if address.starts_with("https://") => (h, "/control"),
        _ => (bare, ""),
    };
    let (host, at) = bare.split_once(':').unwrap_or((bare, ""));
    let host_ok = !host.is_empty() && host.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-');
    let port_ok = at.is_empty() || at.parse::<u16>().is_ok_and(|p| p > 0);
    if !host_ok || !port_ok {
        return Err("That invite has an address that isn't a computer's name or IP.".into());
    }
    if address.starts_with("https://") {
        // a tunnel's own name, or a server's website status address, with a real
        // certificate: checked the usual way (with a time limit, so one that never answers
        // can't hold anything up)
        return Ok((format!("https://{bare}{path}"), tunnel_client()?));
    }
    if address.matches(':').count() > 1 || address.starts_with('[') {
        return Err("IPv6 addresses aren't used for sharing yet.".into());
    }
    let hostport = if address.contains(':') { address.to_string() } else { format!("{address}:{port}") };
    Ok((format!("https://{hostport}"), client(fingerprint)?))
}

// ---- accepting an invite ----------------------------------------------------------------

/// Accepts an invite: tries the owner's addresses in order, keeps the server.
pub fn accept(invite: &str, name: &str) -> Result<Shared, String> {
    let t: InviteText = decode_invite(invite).ok_or("That isn't a Loner-Strike invite. Copy the whole thing the owner sent (it starts with LS1.).")?;
    // the list it goes into can be read and saved, before the invite (which works once) is used
    read_all()?;
    let _ = std::fs::create_dir_all(crate::sys::app_dir());
    let mut last = String::from("Couldn't reach the owner's Loner-Strike.");
    for a in &t.a {
        let Ok((base, c)) = endpoint(a, t.p, &t.f) else { continue };
        let r = c.post(format!("{base}/api/control/redeem")).json(&json!({"invite": t.c, "name": name, "version": VERSION})).send();
        let v: Value = match r {
            Ok(r) => r.json().map_err(|_| "The owner's Loner-Strike answered strangely. Try again.".to_string())?,
            // only when it never got there is the next address tried: an invite works once
            Err(e) if e.is_connect() => {
                last = format!("Couldn't reach the owner's Loner-Strike at {a}. It has to be open, and reachable from here.");
                continue;
            }
            Err(_) => return Err(format!("The owner's Loner-Strike at {a} didn't answer in time. Check with them whether you're in their list; if not, ask for a new invite.")),
        };
        if let Some(e) = v["error"].as_str() {
            return Err(e.to_string());
        }
        let key = v["key"].as_str().filter(|k| k.len() == 64).ok_or("The owner's Loner-Strike answered strangely. Try again.")?;
        let role: Role = serde_json::from_value(v["role"].clone()).map_err(|_| "The owner's Loner-Strike answered strangely. Try again.")?;
        let remote = v["server"]["id"].as_str().unwrap_or("").to_string();
        let _g = FILE.lock().unwrap_or_else(|e| e.into_inner());
        let mut all = read_all()?;
        let s = Shared {
            id: format!("{PREFIX}{}", super::random_hex(4)),
            remote,
            name: v["server"]["name"].as_str().unwrap_or(&t.s).chars().take(64).collect(),
            game: v["server"]["game"].as_str().filter(|g| ["cstrike", "czero"].contains(g)).unwrap_or("cstrike").to_string(),
            role,
            // the address that worked first
            addresses: std::iter::once(a.clone()).chain(t.a.iter().filter(|x| *x != a).cloned()).map(|x| if x.contains(':') || x.starts_with("https://") { x } else { format!("{x}:{}", t.p) }).collect(),
            fingerprint: t.f.clone(),
            key: key.to_string(),
            added: crate::sys::stamp(),
        };
        all.push(s.clone());
        save(&all)?;
        return Ok(s);
    }
    Err(last)
}

/// Takes a shared server off this app's list.
pub fn leave(id: &str) -> Result<(), String> {
    let _g = FILE.lock().unwrap_or_else(|e| e.into_inner());
    let mut all = read_all()?;
    let n = all.len();
    all.retain(|s| s.id != id);
    if all.len() == n {
        return Err("That shared server isn't in your list.".into());
    }
    save(&all)
}

// ---- talking to the owner's app ---------------------------------------------------------

/// One request to the owner's app for this shared server, trying its addresses in order.
/// Returns (status, content type, body).
pub fn call(s: &Shared, method: &str, path_and_query: &str, content_type: Option<&str>, body: Bytes) -> Result<(u16, String, Bytes), String> {
    let last = format!("Couldn't reach the owner's Loner-Strike for {}. It has to be open, and reachable from here.", s.name);
    for a in &s.addresses {
        let Ok((base, c)) = endpoint(a, super::DEFAULT_PORT, &s.fingerprint) else { continue };
        let url = format!("{base}{path_and_query}");
        let mut rb = match method {
            "POST" => c.post(url).body(body.clone()),
            _ => c.get(url),
        };
        rb = rb.header("authorization", format!("Bearer {}", s.key)).header("x-ls-version", VERSION).header("x-panel", "1");
        if let Some(ct) = content_type {
            rb = rb.header("content-type", ct);
        }
        match rb.send() {
            Ok(r) => {
                let status = r.status().as_u16();
                // only data and pictures from an owner's app (never a page that would run here)
                let ct = r.headers().get("content-type").and_then(|v| v.to_str().ok()).unwrap_or("application/json").to_string();
                let ok = ct.starts_with("application/json") || ct.starts_with("text/plain") || ct.starts_with("audio/") || ["image/png", "image/jpeg", "image/webp", "image/gif"].iter().any(|t| ct.starts_with(t));
                let ct = if ok { ct } else { "application/octet-stream".to_string() };
                use std::io::Read;
                let mut bytes = Vec::new();
                r.take(MAX_REPLY).read_to_end(&mut bytes).map_err(|e| e.to_string())?;
                return Ok((status, ct, Bytes::from(bytes)));
            }
            // the next address only when this one was never reached: a change that may have
            // got there isn't sent twice
            Err(e) if e.is_connect() => continue,
            Err(_) => return Err(format!("The owner's Loner-Strike for {} didn't answer in time.", s.name)),
        }
    }
    Err(last)
}

/// A request from this app's window about a shared server, passed on to the owner's app.
/// `local_id`: the shared server's id here; `path`: the request's path and query, with any
/// mention of the local id swapped for the owner's id for it.
pub async fn forward(local_id: String, method: axum::http::Method, path_and_query: String, headers: HeaderMap, body: Body) -> Response {
    let Some(s) = get(&local_id) else {
        return (StatusCode::NOT_FOUND, axum::Json(json!({"error": "That shared server isn't in your list any more."}))).into_response();
    };
    if method != axum::http::Method::GET && method != axum::http::Method::POST {
        return (StatusCode::METHOD_NOT_ALLOWED, axum::Json(json!({"error": "Not allowed."}))).into_response();
    }
    let Ok(bytes) = axum::body::to_bytes(body, 8 * 1024 * 1024).await else {
        return (StatusCode::PAYLOAD_TOO_LARGE, axum::Json(json!({"error": "That's too big to send."}))).into_response();
    };
    let path = path_and_query.replacen(&format!("/api/servers/{local_id}"), &format!("/api/servers/{}", s.remote), 1);
    let ct = headers.get("content-type").and_then(|v| v.to_str().ok()).map(String::from);
    let m = method.as_str().to_string();
    let r = tokio::task::spawn_blocking(move || call(&s, &m, &path, ct.as_deref(), bytes)).await.unwrap_or_else(|_| Err("Loner-Strike hit an internal error.".into()));
    match r {
        Ok((status, ct, bytes)) => {
            // refused (taken away, another version, not this role): the owner's reason, as the
            // app shows a game that isn't running, rather than "lost contact with Loner-Strike"
            if matches!(status, 401 | 403 | 426 | 503) {
                let why = serde_json::from_slice::<Value>(&bytes).ok().and_then(|v| v["error"].as_str().map(String::from)).unwrap_or_else(|| "The owner's Loner-Strike refused.".into());
                return (StatusCode::OK, axum::Json(json!({"online": false, "error": why, "reason": why}))).into_response();
            }
            let status = StatusCode::from_u16(status).unwrap_or(StatusCode::BAD_GATEWAY);
            let mut res = Response::new(Body::from(bytes));
            *res.status_mut() = status;
            if let Ok(v) = axum::http::HeaderValue::from_str(&ct) {
                res.headers_mut().insert(axum::http::header::CONTENT_TYPE, v);
            }
            res
        }
        // the owner's app isn't reachable: said like the app says a game isn't running
        Err(e) => (StatusCode::OK, axum::Json(json!({"online": false, "error": e, "reason": e}))).into_response(),
    }
}

// ---- in this app's server list ------------------------------------------------------------

/// Last answers from owners' apps, per shared server, so the list (asked every few seconds)
/// doesn't ask each owner each time
static VIEWS: Mutex<Option<HashMap<String, (Instant, Value)>>> = Mutex::new(None);
const VIEW_FOR: Duration = Duration::from_secs(8);

/// The shared servers as the server list shows them (the owner's answer, or the last one
/// with why it isn't fresh).
/// Asks the owner's app how its server is now, and keeps the answer for list().
fn refresh(s: &Shared) {
    let v = match call(s, "GET", "/api/control/me", None, Bytes::new()) {
        Ok((status, _, b)) => {
            let me = serde_json::from_slice::<Value>(&b).unwrap_or(Value::Null);
            match me["error"].as_str() {
                // refused, or why it can't (that server isn't there any more)
                Some(why) => json!({"shared": {"role": s.role, "reachable": false, "why": why}}),
                None if status == 200 && me["server"].is_object() => {
                    let mut view = me["server"].clone();
                    view["shared"] = json!({"role": me["role"], "reachable": true});
                    view
                }
                None => json!({"shared": {"role": s.role, "reachable": false, "why": "The owner's Loner-Strike answered strangely."}}),
            }
        }
        Err(why) => json!({"shared": {"role": s.role, "reachable": false, "why": why}}),
    };
    VIEWS.lock().unwrap_or_else(|e| e.into_inner()).get_or_insert_with(HashMap::new).insert(s.id.clone(), (Instant::now(), v));
}

/// Shared servers being asked about right now (one question at a time each)
static ASKING: Mutex<Option<std::collections::HashSet<String>>> = Mutex::new(None);

pub fn list() -> Vec<Value> {
    load()
        .into_iter()
        .map(|s| {
            let cached = VIEWS.lock().unwrap_or_else(|e| e.into_inner()).as_ref().and_then(|m| m.get(&s.id).cloned());
            let fresh = cached.as_ref().is_some_and(|(at, _)| at.elapsed() < VIEW_FOR);
            // the list never waits for an owner's app (it may be off): it shows the last
            // answer, and a new one is asked for in the background
            let start = !fresh && ASKING.lock().unwrap_or_else(|e| e.into_inner()).get_or_insert_with(Default::default).insert(s.id.clone());
            if start {
                let s2 = s.clone();
                std::thread::spawn(move || {
                    refresh(&s2);
                    if let Some(a) = ASKING.lock().unwrap_or_else(|e| e.into_inner()).as_mut() {
                        a.remove(&s2.id);
                    }
                });
            }
            let v = cached.map(|(_, v)| v).unwrap_or_else(|| json!({"state": "checking", "ready": true, "shared": {"role": s.role, "reachable": true}}));
            let mut out = if v.is_object() { v } else { json!({}) };
            // this app's own id and what it knows itself
            out["id"] = json!(s.id);
            out["name"] = out.get("name").cloned().filter(|n| n.is_string()).unwrap_or(json!(s.name));
            out["game"] = out.get("game").cloned().filter(|n| n.is_string()).unwrap_or(json!(s.game));
            if out.get("state").is_none() {
                out["state"] = json!("stopped");
                out["ready"] = json!(true);
            }
            if out["shared"]["reachable"] == json!(false) {
                out["unreachable"] = out["shared"]["why"].clone();
            }
            out
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn addresses() {
        // a server's own shared control: its https:// address and /control only
        assert_eq!(endpoint("https://status.example.com/control", 27030, "").map(|(b, _)| b), Ok("https://status.example.com/control".to_string()));
        assert_eq!(endpoint("https://share.example.com/", 27030, "").map(|(b, _)| b), Ok("https://share.example.com".to_string()));
        for bad in ["https://status.example.com/other", "https://user@evil.com/control", "https://a.com/control/x", "203.0.113.4/control", "https://a.com?x=/control"] {
            assert!(endpoint(bad, 27030, &"a".repeat(64)).is_err(), "{bad}");
        }
    }

    #[test]
    fn shared_ids() {
        assert!(is_shared("sh-1a2b3c4d") && !is_shared("friday-frags"));
    }

    #[test]
    fn what_goes_to_the_owner() {
        for p in ["/api/state", "/api/servers/sh-1/console", "/api/cmd", "/api/gb/jobs", "/thumb/de_dust2.png", "/api/control/me", "/api/app/mode-settings"] {
            assert!(about_the_server(p), "{p}");
        }
        // this app's own: answered here
        for p in ["/api/app", "/api/app/phone", "/api/setup/scan", "/api/servers", "/api/smartchat", "/api/control/accept", "/api/statex"] {
            assert!(!about_the_server(p), "{p}");
        }
    }
}
