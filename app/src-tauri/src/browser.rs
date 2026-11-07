//! The server browser, after Steam's old one: Internet (the Loner-Strike server list: the
//! servers hosts chose to share, see services/serverlist), Favorites, History and LAN.
//!
//! Every server is asked from this computer (A2S over UDP), so the latency is the player's
//! own and the name, map and players are what the server says now. LAN finds games on this
//! network the way the game does: an A2S_INFO broadcast on the usual ports. Favorites and
//! history are kept in browser.json in the app's folder.

use crate::panel_api::from_panel;
use crate::server::Shared;
use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::net::{SocketAddr, ToSocketAddrs, UdpSocket};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// The Loner-Strike server list (services/serverlist)
pub const LIST_URL: &str = "https://list.lonerstrike.com";
const HEADER: [u8; 4] = [0xff; 4];
const SPLIT: [u8; 4] = [0xfe, 0xff, 0xff, 0xff];
const INFO: &[u8] = b"\xff\xff\xff\xffTSource Engine Query\x00";
const PLAYERS: &[u8] = b"\xff\xff\xff\xffU";
const DEFAULT_PORT: u16 = 27015;
/// The ports the game's own LAN search tries
const LAN_PORTS: std::ops::RangeInclusive<u16> = 27015..=27020;
const MAX_HISTORY: usize = 50;
const MAX_FAVORITES: usize = 200;
/// The most packets one split reply may come in
const MAX_PARTS: usize = 16;

// ---- asking a server ------------------------------------------------------------------

#[derive(Serialize, Clone, Debug, PartialEq, Default)]
pub struct Info {
    pub name: String,
    pub map: String,
    /// the game folder: cstrike or czero
    pub game: String,
    pub players: u8,
    pub bots: u8,
    pub max_players: u8,
    pub password: bool,
    pub secure: bool,
}

fn cstr(d: &[u8], i: &mut usize) -> Option<String> {
    let end = d.get(*i..)?.iter().position(|&b| b == 0)? + *i;
    let s = &d[*i..end];
    *i = end + 1;
    // most names are UTF-8; old ones Latin-1
    Some(String::from_utf8(s.to_vec()).unwrap_or_else(|_| s.iter().map(|&b| b as char).collect()))
}

/// An A2S_INFO reply (after the FF FF FF FF header): 'I' today, 'm' from old servers.
pub fn parse_info(d: &[u8]) -> Option<Info> {
    let mut i = 1;
    match d.first()? {
        0x49 => {
            i += 1;
            let name = cstr(d, &mut i)?;
            let map = cstr(d, &mut i)?;
            let game = cstr(d, &mut i)?;
            cstr(d, &mut i)?;
            i += 2;
            let b = d.get(i..i + 7)?;
            Some(Info { name, map, game, players: b[0], max_players: b[1], bots: b[2].min(b[0]), password: b[5] != 0, secure: b[6] != 0 })
        }
        0x6d => {
            cstr(d, &mut i)?;
            let name = cstr(d, &mut i)?;
            let map = cstr(d, &mut i)?;
            let game = cstr(d, &mut i)?;
            cstr(d, &mut i)?;
            let (players, max_players, password) = (*d.get(i)?, *d.get(i + 1)?, *d.get(i + 5)? != 0);
            let mut j = i + 6;
            if *d.get(j)? == 1 {
                j += 1;
                cstr(d, &mut j)?;
                cstr(d, &mut j)?;
                j += 1 + 4 + 4 + 1 + 1;
            } else {
                j += 1;
            }
            let (secure, bots) = (*d.get(j)? != 0, *d.get(j + 1)?);
            Some(Info { name, map, game, players, max_players, bots: bots.min(players), password, secure })
        }
        _ => None,
    }
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Player {
    pub name: String,
    pub score: i32,
    /// seconds on the server
    pub time: u32,
}

/// An A2S_PLAYER reply ('D', after the header). Empty names (people still connecting) left out.
pub fn parse_players(d: &[u8]) -> Option<Vec<Player>> {
    if d.first() != Some(&0x44) {
        return None;
    }
    let count = *d.get(1)? as usize;
    let mut i = 2;
    let mut out = Vec::new();
    for _ in 0..count {
        i += 1;
        let name = cstr(d, &mut i)?;
        let score = i32::from_le_bytes(d.get(i..i + 4)?.try_into().ok()?);
        let time = f32::from_le_bytes(d.get(i + 4..i + 8)?.try_into().ok()?);
        i += 8;
        if !name.is_empty() {
            out.push(Player { name, score, time: time.max(0.0) as u32 });
        }
    }
    Some(out)
}

/// One reply, put back together when GoldSrc split it (the high four bits of the fifth
/// byte are the packet's number, the low four how many there are).
/// The whole reply has to arrive by `deadline`, in MAX_PARTS packets at most.
fn receive(s: &UdpSocket, deadline: Instant) -> Option<Vec<u8>> {
    let mut buf = [0u8; 4096];
    let mut recv = |buf: &mut [u8]| {
        let left = deadline.checked_duration_since(Instant::now()).filter(|d| !d.is_zero())?;
        s.set_read_timeout(Some(left)).ok()?;
        s.recv(buf).ok()
    };
    let n = recv(&mut buf)?;
    if buf[..4] != SPLIT {
        return Some(buf[..n].to_vec());
    }
    let id = buf[4..8].to_vec();
    let mut parts: Vec<Option<Vec<u8>>> = Vec::new();
    let mut data = buf[..n].to_vec();
    for _ in 0..MAX_PARTS {
        if data.len() < 9 || data[..4] != SPLIT || data[4..8] != id[..] {
            return None;
        }
        let (number, total) = ((data[8] >> 4) as usize, (data[8] & 0x0f) as usize);
        if total == 0 || number >= total {
            return None;
        }
        parts.resize(total, None);
        parts[number] = Some(data[9..].to_vec());
        if parts.iter().all(|p| p.is_some()) {
            return Some(parts.into_iter().flatten().flatten().collect());
        }
        let n = recv(&mut buf)?;
        data = buf[..n].to_vec();
    }
    None
}

/// Sends an A2S request, answering a challenge if the server asks for one: the reply's body
/// (after FF FF FF FF) and how long the last round trip took.
fn ask(addr: SocketAddr, request: &[u8], timeout: Duration) -> Option<(Vec<u8>, u32)> {
    let s = UdpSocket::bind(if addr.is_ipv4() { "0.0.0.0:0" } else { "[::]:0" }).ok()?;
    s.set_read_timeout(Some(timeout)).ok()?;
    s.connect(addr).ok()?;
    // A2S_PLAYER starts by asking for a challenge with FF FF FF FF
    let mut challenge: Vec<u8> = if request == PLAYERS { HEADER.to_vec() } else { Vec::new() };
    for _ in 0..3 {
        let sent = Instant::now();
        s.send(&[request, &challenge].concat()).ok()?;
        let reply = receive(&s, sent + timeout)?;
        let ping = sent.elapsed().as_millis() as u32;
        if reply.len() < 5 || reply[..4] != HEADER {
            return None;
        }
        if reply[4] == b'A' && reply.len() >= 9 {
            challenge = reply[5..9].to_vec();
            continue;
        }
        return Some((reply[4..].to_vec(), ping));
    }
    None
}

/// "host" or "host:port" (the port the game uses if left out) as the address to ask.
pub fn resolve(address: &str) -> Option<SocketAddr> {
    let a = clean_address(address)?;
    a.to_socket_addrs().ok()?.find(|s| s.is_ipv4())
}

/// An address someone typed: host[:port], plain characters, with the port added.
pub fn clean_address(text: &str) -> Option<String> {
    let t = text.trim().trim_start_matches("steam://connect/").trim_end_matches('/').to_lowercase();
    let (host, port) = match t.rsplit_once(':') {
        Some((h, p)) => (h.to_string(), p.parse::<u16>().ok().filter(|p| *p > 0)?),
        None => (t.clone(), DEFAULT_PORT),
    };
    let ok = !host.is_empty() && host.len() <= 100 && host.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-');
    ok.then(|| format!("{host}:{port}"))
}

/// A server's details now, and the round trip in ms.
pub fn info(address: &str) -> Option<(Info, u32)> {
    let (body, ping) = ask(resolve(address)?, INFO, Duration::from_millis(1500))?;
    Some((parse_info(&body)?, ping))
}

/// Who's on a server.
pub fn players(address: &str) -> Option<Vec<Player>> {
    let (body, _) = ask(resolve(address)?, PLAYERS, Duration::from_millis(1500))?;
    parse_players(&body)
}

/// Games on this network: an A2S_INFO broadcast on the game's ports (and this computer's
/// own servers), with the replies that come back within `wait`.
pub fn lan_scan(wait: Duration) -> Vec<(String, Info, u32)> {
    let Ok(s) = UdpSocket::bind("0.0.0.0:0") else { return Vec::new() };
    let _ = s.set_broadcast(true);
    let started = Instant::now();
    let mut sent_at = std::collections::HashMap::new();
    for port in LAN_PORTS {
        for host in ["255.255.255.255", "127.0.0.1"] {
            if let Ok(a) = format!("{host}:{port}").parse::<SocketAddr>() {
                let _ = s.send_to(INFO, a);
            }
        }
    }
    let mut found: Vec<(String, Info, u32)> = Vec::new();
    let mut buf = [0u8; 2048];
    while let Some(left) = wait.checked_sub(started.elapsed()) {
        let _ = s.set_read_timeout(Some(left.max(Duration::from_millis(1))));
        let Ok((n, from)) = s.recv_from(&mut buf) else { break };
        let d = &buf[..n];
        if n < 5 || d[..4] != HEADER {
            continue;
        }
        if d[4] == b'A' && n >= 9 {
            // a server that wants the challenge back: ask it again, timing from now
            sent_at.insert(from, Instant::now());
            let _ = s.send_to(&[INFO, &d[5..9]].concat(), from);
            continue;
        }
        if let Some(info) = parse_info(&d[4..]) {
            let ping = sent_at.get(&from).copied().unwrap_or(started).elapsed().as_millis() as u32;
            let addr = from.to_string();
            if !found.iter().any(|(a, ..)| *a == addr) {
                found.push((addr, info, ping));
            }
        }
    }
    found
}

// ---- favorites and history ------------------------------------------------------------

#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
struct Saved {
    favorites: Vec<String>,
    /// most recent first: (address, when, unix seconds)
    history: Vec<(String, u64)>,
}

static FILE: Mutex<()> = Mutex::new(());

fn saved_path() -> std::path::PathBuf {
    crate::sys::app_dir().join("browser.json")
}

fn load() -> Saved {
    std::fs::read_to_string(saved_path()).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

fn store(s: &Saved) -> Result<(), String> {
    let text = serde_json::to_string_pretty(s).map_err(|e| e.to_string())?;
    crate::sys::write_atomic(&saved_path(), text.as_bytes()).map_err(|e| format!("Couldn't save it ({e})."))
}

fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// Remembers a server joined from the browser, most recent first.
fn remember(address: &str) {
    let _l = FILE.lock().unwrap_or_else(|e| e.into_inner());
    let mut s = load();
    s.history.retain(|(a, _)| a != address);
    s.history.insert(0, (address.to_string(), now()));
    s.history.truncate(MAX_HISTORY);
    let _ = store(&s);
}

// ---- the lists ------------------------------------------------------------------------

/// Asks every address at once (a few at a time): the row for each, answered or not.
fn rows(addresses: Vec<(String, Option<Value>)>) -> Vec<Value> {
    let mut out = Vec::new();
    for chunk in addresses.chunks(24) {
        let handles: Vec<_> = chunk.iter().cloned().map(|(a, known)| std::thread::spawn(move || (info(&a), a, known))).collect();
        for h in handles {
            let Ok((found, address, known)) = h.join() else { continue };
            out.push(match found {
                Some((i, ping)) => json!({"address": address, "info": i, "ping": ping, "answered": true}),
                // what the list last knew, greyed out
                None => json!({"address": address, "info": known, "ping": null, "answered": false}),
            });
        }
    }
    out
}

fn internet() -> Result<Vec<Value>, String> {
    let r = crate::sys::http().get(format!("{LIST_URL}/servers")).timeout(Duration::from_secs(8)).send().map_err(|_| "Couldn't reach the Loner-Strike server list. Check your internet connection.".to_string())?;
    let v: Value = r.text().ok().and_then(|t| serde_json::from_str(&t).ok()).ok_or("The server list sent something unexpected. Try again in a minute.")?;
    let list = v["servers"].as_array().cloned().unwrap_or_default();
    let addresses = list
        .into_iter()
        .filter_map(|s| {
            let a = clean_address(s["address"].as_str()?)?;
            Some((a, Some(s)))
        })
        .take(500)
        .collect();
    Ok(rows(addresses))
}

#[derive(Deserialize)]
struct ListQuery {
    tab: String,
}

async fn api_list(Query(q): Query<ListQuery>) -> (StatusCode, Json<Value>) {
    let r = tokio::task::spawn_blocking(move || -> Result<Vec<Value>, String> {
        match q.tab.as_str() {
            "internet" => internet(),
            "lan" => Ok(lan_scan(Duration::from_millis(1500)).into_iter().map(|(a, i, p)| json!({"address": a, "info": i, "ping": p, "answered": true})).collect()),
            "favorites" => Ok(rows(load().favorites.into_iter().map(|a| (a, None)).collect())),
            "history" => {
                let h = load().history;
                let mut list = rows(h.iter().map(|(a, _)| (a.clone(), None)).collect());
                for row in &mut list {
                    if let Some((_, at)) = h.iter().find(|(a, _)| Some(a.as_str()) == row["address"].as_str()) {
                        row["last_played"] = json!(at);
                    }
                }
                Ok(list)
            }
            _ => Err("Unknown tab.".into()),
        }
    })
    .await
    .unwrap_or_else(|_| Err("Loner-Strike hit an internal error. Try again.".into()));
    let favorites = load().favorites;
    match r {
        Ok(list) => (StatusCode::OK, Json(json!({"servers": list, "favorites": favorites}))),
        Err(e) => (StatusCode::OK, Json(json!({"error": e, "servers": [], "favorites": favorites}))),
    }
}

#[derive(Deserialize)]
struct AddressQuery {
    address: String,
}

async fn api_server(Query(q): Query<AddressQuery>) -> (StatusCode, Json<Value>) {
    let Some(address) = clean_address(&q.address) else { return (StatusCode::OK, Json(json!({"error": "That isn't a server address."}))) };
    let v = tokio::task::spawn_blocking(move || {
        let found = info(&address);
        let players = found.as_ref().and_then(|_| players(&address));
        match found {
            Some((i, ping)) => json!({"address": address, "info": i, "ping": ping, "players": players}),
            None => json!({"address": address, "error": "The server didn't answer."}),
        }
    })
    .await
    .unwrap_or_else(|_| json!({"error": "Loner-Strike hit an internal error. Try again."}));
    (StatusCode::OK, Json(v))
}

#[derive(Deserialize)]
struct FavoriteReq {
    address: String,
    on: bool,
}

async fn api_favorite(headers: HeaderMap, Json(req): Json<FavoriteReq>) -> (StatusCode, Json<Value>) {
    if !from_panel(&headers) {
        return (StatusCode::FORBIDDEN, Json(json!({"error": "forbidden"})));
    }
    let Some(address) = clean_address(&req.address) else { return (StatusCode::OK, Json(json!({"error": "Type an address like play.example.com or 1.2.3.4:27015."}))) };
    let _l = FILE.lock().unwrap_or_else(|e| e.into_inner());
    let mut s = load();
    s.favorites.retain(|a| *a != address);
    if req.on {
        if s.favorites.len() >= MAX_FAVORITES {
            return (StatusCode::OK, Json(json!({"error": format!("Keep it to {MAX_FAVORITES} favorites.")})));
        }
        s.favorites.push(address);
    }
    match store(&s) {
        Ok(()) => (StatusCode::OK, Json(json!({"ok": true, "favorites": s.favorites}))),
        Err(e) => (StatusCode::OK, Json(json!({"error": e}))),
    }
}

#[derive(Deserialize)]
struct ConnectReq {
    address: String,
    #[serde(default)]
    password: String,
    /// the game folder the server runs (cstrike or czero)
    #[serde(default)]
    game: String,
}

/// Joins a server: the open game is told to through Steam, otherwise the game starts on it.
async fn api_connect(State(st): State<Shared>, headers: HeaderMap, Json(req): Json<ConnectReq>) -> (StatusCode, Json<Value>) {
    if !from_panel(&headers) {
        return (StatusCode::FORBIDDEN, Json(json!({"error": "forbidden"})));
    }
    let r = tokio::task::spawn_blocking(move || -> Result<Value, String> {
        let address = clean_address(&req.address).ok_or("That isn't a server address.")?;
        let password = req.password.trim().to_string();
        if !password.is_empty() && !crate::sharing::password_ok(&password) {
            return Err("Passwords can use letters, numbers, - and _ (up to 24).".into());
        }
        let install = crate::servers::client(&st).ok_or("Couldn't find the game to join with.")?;
        let game = if req.game == "czero" { "czero" } else { "cstrike" };
        if !install.games.iter().any(|g| g == game) {
            return Err(format!("This server runs {}, which isn't installed here.", if game == "czero" { "Condition Zero" } else { "Counter-Strike 1.6" }));
        }
        remember(&address);
        if crate::sys::game_running() {
            let url = if password.is_empty() { format!("steam://connect/{address}") } else { format!("steam://connect/{address}/{password}") };
            crate::steam::open_steam_url(&install.hl, &url)?;
        } else {
            let mut args = Vec::new();
            if !password.is_empty() {
                args.extend(["+password".to_string(), password]);
            }
            args.extend(["+connect".to_string(), address]);
            crate::steam::launch(&install.hl, if game == "czero" { "80" } else { "10" }, &args)?;
        }
        Ok(json!({"ok": true}))
    })
    .await
    .unwrap_or_else(|_| Err("Loner-Strike hit an internal error. Try again.".into()));
    match r {
        Ok(v) => (StatusCode::OK, Json(v)),
        Err(e) => (StatusCode::OK, Json(json!({"error": e}))),
    }
}

// ---- listing a server -------------------------------------------------------------------

/// Asks the Loner-Strike server list to take a server in (it checks the server itself, so
/// the server has to be running with ls_list 1 and reachable from the internet).
pub fn register(address: &str) -> Result<(), String> {
    let r = crate::sys::http()
        .post(format!("{LIST_URL}/register"))
        .timeout(Duration::from_secs(15))
        .header("Content-Type", "application/json")
        .body(json!({"address": address}).to_string())
        .send()
        .map_err(|_| "Couldn't reach the Loner-Strike server list.".to_string())?;
    let v: Value = r.text().ok().and_then(|t| serde_json::from_str(&t).ok()).ok_or("The server list sent something unexpected.")?;
    if v["ok"].as_bool() == Some(true) {
        Ok(())
    } else {
        Err(v["error"].as_str().unwrap_or("The server list said no.").to_string())
    }
}

pub fn routes() -> Router<Shared> {
    Router::new()
        .route("/api/browser/list", get(api_list))
        .route("/api/browser/server", get(api_server))
        .route("/api/browser/favorite", post(api_favorite))
        .route("/api/browser/connect", post(api_connect))
}

#[cfg(test)]
mod tests {
    use super::*;

    const INFO_I: &[u8] = b"I\x30Pub \xc3\xa9\x00de_dust2\x00cstrike\x00Counter-Strike\x00\x0a\x00\x05\x20\x03dl\x01\x01\x01";

    #[test]
    fn info_replies() {
        let i = parse_info(INFO_I).unwrap();
        assert_eq!(i, Info { name: "Pub é".into(), map: "de_dust2".into(), game: "cstrike".into(), players: 5, bots: 3, max_players: 32, password: true, secure: true });
        let old = b"m1.2.3.4:27015\x00Old \xe9 box\x00cs_office\x00czero\x00CZ\x00\x04\x10\x2fdl\x00\x00\x01\x02";
        let o = parse_info(old).unwrap();
        assert_eq!((o.name.as_str(), o.game.as_str(), o.players, o.bots, o.password, o.secure), ("Old é box", "czero", 4, 2, false, true));
        assert!(parse_info(b"I\x30cut").is_none() && parse_info(b"").is_none() && parse_info(b"Z").is_none());
    }

    #[test]
    fn player_replies() {
        let mut d = b"D\x03".to_vec();
        for (name, score, time) in [("Alpha", 12i32, 61.5f32), ("", 0, 1.0), ("Bot Joe", -2, 300.0)] {
            d.push(0);
            d.extend(name.as_bytes());
            d.push(0);
            d.extend(score.to_le_bytes());
            d.extend(time.to_le_bytes());
        }
        let p = parse_players(&d).unwrap();
        assert_eq!(p, vec![Player { name: "Alpha".into(), score: 12, time: 61 }, Player { name: "Bot Joe".into(), score: -2, time: 300 }]);
        assert!(parse_players(b"D\x02\x00cut").is_none());
    }

    #[test]
    fn addresses() {
        assert_eq!(clean_address(" Play.Example.com ").as_deref(), Some("play.example.com:27015"));
        assert_eq!(clean_address("steam://connect/1.2.3.4:27016/").as_deref(), Some("1.2.3.4:27016"));
        for bad in ["", "a b", "x:0", "x:99999", "x;quit:27015", "\"x\""] {
            assert!(clean_address(bad).is_none(), "{bad}");
        }
    }

    /// A server on 127.0.0.1 that wants a challenge and splits its player list in two.
    #[test]
    fn asking_with_challenges_and_split_replies() {
        let s = UdpSocket::bind("127.0.0.1:0").unwrap();
        let addr = s.local_addr().unwrap();
        let mut reply = b"\xff\xff\xff\xffD\x01\x00Alpha\x00".to_vec();
        reply.extend(5i32.to_le_bytes());
        reply.extend(10f32.to_le_bytes());
        std::thread::spawn(move || {
            let mut buf = [0u8; 1500];
            for _ in 0..4 {
                let Ok((n, from)) = s.recv_from(&mut buf) else { return };
                let d = &buf[..n];
                if d.ends_with(b"CHAL") {
                    if d.starts_with(INFO) {
                        let _ = s.send_to(&[&HEADER[..], INFO_I].concat(), from);
                    } else {
                        let half = reply.len() / 2;
                        let _ = s.send_to(&[&SPLIT[..], b"\x01\x00\x00\x00", &[0x12], &reply[half..]].concat(), from);
                        let _ = s.send_to(&[&SPLIT[..], b"\x01\x00\x00\x00", &[0x02], &reply[..half]].concat(), from);
                    }
                } else {
                    let _ = s.send_to(b"\xff\xff\xff\xffACHAL", from);
                }
            }
        });
        let a = addr.to_string();
        assert_eq!(info(&a).unwrap().0.map, "de_dust2");
        assert_eq!(players(&a).unwrap(), vec![Player { name: "Alpha".into(), score: 5, time: 10 }]);
    }
}
