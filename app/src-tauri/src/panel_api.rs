//! The game control API: same requests and answers as panel/panel.py, so the
//! web panel and the app share one protocol.

use crate::game::{CmdError, Game};
use crate::gamebanana::StoreError;
use crate::server::Shared;
use axum::body::Bytes;
use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::{json, Value};
use std::collections::HashMap;

fn no_game() -> Value {
    json!({"online": false, "reason": "Couldn't find Condition Zero or Counter-Strike 1.6 on this computer."})
}

fn reply(code: StatusCode, v: Value) -> Response {
    (code, [(header::CACHE_CONTROL, "no-store")], Json(v)).into_response()
}

fn forbidden() -> Response {
    reply(StatusCode::FORBIDDEN, json!({"error": "forbidden"}))
}

fn no_game_found() -> Response {
    reply(StatusCode::SERVICE_UNAVAILABLE, json!({"ok": false, "error": "No game found."}))
}

/// Runs f with the game on a blocking thread (file scans, RCON and HTTPS all block).
async fn with_game<T: Send + 'static>(st: &Shared, f: impl FnOnce(&Game) -> T + Send + 'static) -> Option<T> {
    let g = st.game()?;
    tokio::task::spawn_blocking(move || f(&g)).await.ok()
}

/// Only the app's own pages send X-Panel; another site can't add it without a CORS preflight.
pub fn from_panel(headers: &HeaderMap) -> bool {
    headers.get("x-panel").and_then(|v| v.to_str().ok()) == Some("1")
}

/// A POST body as JSON (like panel.py: at most 4 KB, empty means {}).
fn body_json(body: &Bytes) -> Result<Value, Response> {
    let b = &body[..body.len().min(4096)];
    if b.iter().all(|c| c.is_ascii_whitespace()) {
        return Ok(json!({}));
    }
    match serde_json::from_slice::<Value>(b) {
        Ok(v @ Value::Object(_)) => Ok(v),
        Ok(_) => Err(reply(StatusCode::BAD_REQUEST, json!({"ok": false, "error": "expected a JSON object"}))),
        Err(e) => Err(reply(StatusCode::BAD_REQUEST, json!({"ok": false, "error": e.to_string()}))),
    }
}

/// Python's str(x) for the strings the panel sends.
fn text(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => "None".into(),
        v => v.to_string(),
    }
}

/// Python's int(x) for ids: numbers or numeric strings.
fn int(v: &Value) -> Option<i64> {
    match v {
        Value::Number(n) => n.as_i64(),
        Value::String(s) => s.trim().parse().ok(),
        _ => None,
    }
}

/// Python's bool(x).
fn truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().map(|f| f != 0.0).unwrap_or(false),
        Value::String(s) => !s.is_empty(),
        Value::Array(a) => !a.is_empty(),
        Value::Object(o) => !o.is_empty(),
    }
}

// ---- live game ---------------------------------------------------------------

async fn api_state(State(st): State<Shared>) -> Json<Value> {
    Json(with_game(&st, |g| g.state()).await.unwrap_or_else(no_game))
}

async fn api_health(State(st): State<Shared>) -> Json<Value> {
    Json(with_game(&st, |g| g.health()).await.unwrap_or_else(|| json!({"ok": false, "checks": [], "fixed": []})))
}

async fn api_cmd(State(st): State<Shared>, headers: HeaderMap, body: Bytes) -> Response {
    if !from_panel(&headers) {
        return forbidden();
    }
    let req = match body_json(&body) {
        Ok(v) => v,
        Err(r) => return r,
    };
    let cmd = text(req.get("cmd").unwrap_or(&json!("")));
    let args: Vec<String> = req["args"].as_array().into_iter().flatten().map(text).collect();
    match with_game(&st, move |g| g.command(&cmd, &args)).await {
        None => no_game_found(),
        Some(Ok(lines)) => reply(StatusCode::OK, json!({"ok": true, "output": lines})),
        Some(Err(CmdError::Bad(e))) => reply(StatusCode::BAD_REQUEST, json!({"ok": false, "error": e})),
        Some(Err(CmdError::Game(e))) => reply(StatusCode::SERVICE_UNAVAILABLE, json!({"ok": false, "error": e})),
    }
}

// ---- maps --------------------------------------------------------------------------

async fn api_maps(State(st): State<Shared>, Query(q): Query<HashMap<String, String>>) -> Response {
    let refresh = q.contains_key("refresh");
    let maps = with_game(&st, move |g| g.maps(refresh)).await.unwrap_or_default();
    reply(StatusCode::OK, json!(maps))
}

async fn api_mapflag(State(st): State<Shared>, headers: HeaderMap, body: Bytes) -> Response {
    if !from_panel(&headers) {
        return forbidden();
    }
    let req = match body_json(&body) {
        Ok(v) => v,
        Err(r) => return r,
    };
    let flag = text(req.get("flag").unwrap_or(&json!("")));
    let map = text(req.get("map").unwrap_or(&json!("")));
    let on = truthy(&req["on"]);
    match with_game(&st, move |g| g.set_flag(&flag, &map, on)).await {
        None => no_game_found(),
        Some(Ok(())) => reply(StatusCode::OK, json!({"ok": true})),
        Some(Err(e)) => reply(StatusCode::BAD_REQUEST, json!({"ok": false, "error": e})),
    }
}

async fn thumb(State(st): State<Shared>, Path(file): Path<String>) -> Response {
    let name = file.strip_suffix(".png").unwrap_or("").to_string();
    if !crate::maps::safe_arg(&name) {
        return reply(StatusCode::NOT_FOUND, json!({"error": "not found"}));
    }
    let png = with_game(&st, move |g| g.thumbnail(&name).and_then(|(p, kind)| std::fs::read(p).ok().map(|d| (d, kind)))).await.flatten();
    match png {
        Some((data, kind)) => (
            [(header::CONTENT_TYPE, "image/png"), (header::CACHE_CONTROL, "max-age=3600"), (header::HeaderName::from_static("x-thumb-kind"), kind)],
            data,
        )
            .into_response(),
        None => (StatusCode::NOT_FOUND, [(header::CONTENT_TYPE, "text/plain")], "no picture").into_response(),
    }
}

// ---- GameBanana map store ------------------------------------------------------------

fn store_error(e: StoreError, ok_field: bool) -> Response {
    let code = match e {
        StoreError::Bad(_) => StatusCode::BAD_REQUEST,
        StoreError::Net => StatusCode::BAD_GATEWAY,
    };
    let v = if ok_field { json!({"ok": false, "error": e.message()}) } else { json!({"error": e.message()}) };
    reply(code, v)
}

async fn gb_browse(State(st): State<Shared>, Query(q): Query<HashMap<String, String>>) -> Response {
    // like parse_qs: blank values count as missing
    let get = |k: &str| q.get(k).filter(|v| !v.is_empty()).cloned();
    let num = |k: &str, d: i64| match get(k) {
        None => Ok(d),
        Some(v) => v.trim().parse::<i64>().map_err(|_| format!("invalid number for {k}")),
    };
    let (cat, page) = match (num("cat", -1), num("page", 1)) {
        (Ok(c), Ok(p)) => ((c >= 0).then_some(c), p),
        (Err(e), _) | (_, Err(e)) => return reply(StatusCode::BAD_REQUEST, json!({"error": e})),
    };
    let section = get("section").unwrap_or_else(|| "maps".into());
    let sort = get("sort").unwrap_or_else(|| "popular".into());
    let query = get("q").unwrap_or_default();
    match with_game(&st, move |g| g.store.browse(&section, cat, &sort, &query, page)).await {
        None => no_game_found(),
        Some(Ok(v)) => reply(StatusCode::OK, v),
        Some(Err(e)) => store_error(e, false),
    }
}

async fn gb_mod(State(st): State<Shared>, Path(id): Path<String>, Query(q): Query<HashMap<String, String>>) -> Response {
    let section = q.get("section").filter(|v| !v.is_empty()).cloned().unwrap_or_else(|| "maps".into());
    if id.is_empty() || id.len() > 9 || !id.bytes().all(|c| c.is_ascii_digit()) {
        return reply(StatusCode::NOT_FOUND, json!({"error": "not found"}));
    }
    let id: i64 = id.parse().unwrap_or(0);
    match with_game(&st, move |g| g.store.details(&section, id)).await {
        None => no_game_found(),
        Some(Ok(v)) => reply(StatusCode::OK, v),
        Some(Err(e)) => store_error(e, false),
    }
}

async fn gb_jobs(State(st): State<Shared>) -> Response {
    let jobs = st.game().map(|g| g.store.job_list()).unwrap_or_default();
    reply(StatusCode::OK, json!(jobs))
}

async fn gb_install(State(st): State<Shared>, headers: HeaderMap, body: Bytes) -> Response {
    if !from_panel(&headers) {
        return forbidden();
    }
    let req = match body_json(&body) {
        Ok(v) => v,
        Err(r) => return r,
    };
    let (Some(m), Some(f)) = (int(&req["mod"]), int(&req["file"])) else {
        return reply(StatusCode::BAD_REQUEST, json!({"ok": false, "error": "invalid mod or file"}));
    };
    let section = req.get("section").map(text).unwrap_or_else(|| "maps".into());
    // skins, sounds and the rest only change how the game looks to you: always your game
    let g = if section == "maps" { st.game() } else { st.local_game() };
    match g.map(|g| g.store.start_section_install(&section, m, f)) {
        None => no_game_found(),
        Some(Ok(job)) => reply(StatusCode::OK, json!({"ok": true, "job": job})),
        Some(Err(e)) => store_error(e, true),
    }
}

async fn gb_uninstall(State(st): State<Shared>, headers: HeaderMap, body: Bytes) -> Response {
    if !from_panel(&headers) {
        return forbidden();
    }
    let req = match body_json(&body) {
        Ok(v) => v,
        Err(r) => return r,
    };
    let Some(m) = int(&req["mod"]) else {
        return reply(StatusCode::BAD_REQUEST, json!({"ok": false, "error": "invalid mod"}));
    };
    let section = req.get("section").map(text).unwrap_or_else(|| "maps".into());
    let Some(g) = (if section == "maps" { st.game() } else { st.local_game() }) else {
        return no_game_found();
    };
    match tokio::task::spawn_blocking(move || g.store.uninstall_section(&section, m)).await.ok() {
        None => no_game_found(),
        Some(Ok(mut v)) => {
            v["ok"] = json!(true);
            reply(StatusCode::OK, v)
        }
        Some(Err(e)) => store_error(e, true),
    }
}

// ---- playing with friends ---------------------------------------------------------------

async fn share_get(State(st): State<Shared>) -> Response {
    match with_game(&st, |g| g.sharing.status()).await {
        Some(v) => reply(StatusCode::OK, v),
        None => no_game_found(),
    }
}

async fn share_post(State(st): State<Shared>, headers: HeaderMap, body: Bytes) -> Response {
    if !from_panel(&headers) {
        return forbidden();
    }
    let req = match body_json(&body) {
        Ok(v) => v,
        Err(r) => return r,
    };
    let res = with_game(&st, move |g| {
        let sh = &g.sharing;
        let s = |k: &str| req.get(k).map(text).unwrap_or_default();
        let done = match req["action"].as_str().unwrap_or("") {
            "server" => sh.set_server(&s("name"), &s("password")),
            "method" => sh.set_method(&s("method")),
            "connect" => {
                sh.connect();
                Ok(())
            }
            "cancel" => {
                sh.cancel();
                Ok(())
            }
            "disconnect" => {
                sh.disconnect();
                Ok(())
            }
            "start" => sh.start(),
            "stop" => {
                sh.stop();
                Ok(())
            }
            _ => Err("unknown action".to_string()),
        };
        done.map(|()| {
            let mut v = sh.status();
            v["ok"] = json!(true);
            v
        })
    })
    .await;
    match res {
        None => no_game_found(),
        Some(Ok(v)) => reply(StatusCode::OK, v),
        Some(Err(e)) => reply(StatusCode::BAD_REQUEST, json!({"ok": false, "error": e})),
    }
}

pub fn routes() -> Router<Shared> {
    Router::new()
        .route("/api/state", get(api_state))
        .route("/api/health", get(api_health))
        .route("/api/cmd", post(api_cmd))
        .route("/api/maps", get(api_maps))
        .route("/api/mapflag", post(api_mapflag))
        .route("/thumb/:file", get(thumb))
        .route("/api/gb/browse", get(gb_browse))
        .route("/api/gb/jobs", get(gb_jobs))
        .route("/api/gb/mod/:id", get(gb_mod))
        .route("/api/gb/install", post(gb_install))
        .route("/api/gb/uninstall", post(gb_uninstall))
        .route("/api/share", get(share_get).post(share_post))
}

#[cfg(test)]
mod tests {
    use crate::server::{self, AppState, Listener};
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use serde_json::{json, Value};
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex, OnceLock, RwLock};
    use tower::ServiceExt;

    const HOST: &str = "127.0.0.1:8027";

    fn fixture() -> (PathBuf, server::Shared) {
        let root = crate::sys::make_temp_dir(&std::env::temp_dir().join("loner-strike-tests"), "api-").unwrap();
        let hl = root.join("Half-Life");
        let app = root.join("app");
        for d in ["czero/maps", "cstrike/maps", "czero/gfx/thumbnails/maps", "cstrike/overviews"] {
            std::fs::create_dir_all(hl.join(d)).unwrap();
        }
        std::fs::write(hl.join("czero/maps/de_alpha.bsp"), b"").unwrap();
        std::fs::write(hl.join("czero/maps/de_alpha.nav"), b"").unwrap();
        std::fs::write(hl.join("cstrike/maps/aim_beta.bsp"), b"").unwrap();
        std::fs::write(hl.join("cstrike/maps/DE_ALPHA.bsp"), b"").unwrap(); // same map, other folder
        std::fs::write(hl.join("cstrike/maps/de_dust2.bsp"), b"").unwrap(); // CS 1.6's own
        std::fs::write(hl.join("cstrike/maps/fun_added.bsp"), b"").unwrap(); // put there by hand
        // a 4x2 screenshot as TGA and a 1024x600 radar as BMP
        image::RgbImage::from_pixel(4, 2, image::Rgb([200, 10, 10])).save(hl.join("czero/gfx/thumbnails/maps/de_alpha.tga")).unwrap();
        image::RgbImage::from_pixel(1024, 600, image::Rgb([0, 0, 0])).save(hl.join("cstrike/overviews/aim_beta.bmp")).unwrap();
        // no network in tests: say GameBanana had no picture for these
        let shots = app.join("cache/gbshots");
        std::fs::create_dir_all(&shots).unwrap();
        let now = crate::sys::now();
        std::fs::write(shots.join("not_found.json"), json!({"de_alpha": now, "aim_beta": now, "de_gamma": now}).to_string()).unwrap();
        std::fs::write(app.join("map_sources.json"), json!({"groups": [{"name": "Valve", "maps": ["de_alpha"]}]}).to_string()).unwrap();
        std::fs::write(
            app.join("installed_maps.json"),
            json!({"42": {"name": "Beta", "author": "Someone", "maps": ["aim_beta"], "files": ["maps/aim_beta.bsp"], "shared": []}}).to_string(),
        )
        .unwrap();
        let game = crate::game::Game::with_app_dir(&hl, "czero", &app);
        let st = Arc::new(AppState {
            current: RwLock::new(Some(Arc::new(game))),
            hosts: RwLock::new(vec![HOST.into()]),
            lan_hosts: RwLock::new(vec![]),
            token: "testtoken0123456789".into(),
            app: OnceLock::new(),
            phone: Mutex::new(crate::app_api::Phone::default()),
        });
        (root, st)
    }

    async fn call(st: &server::Shared, method: &str, uri: &str, body: Option<Value>, panel: bool) -> (StatusCode, Vec<u8>, String) {
        let mut b = Request::builder().method(method).uri(uri).header("host", HOST);
        if panel {
            b = b.header("x-panel", "1");
        }
        let req = match body {
            Some(v) => b.header("content-type", "application/json").body(Body::from(v.to_string())).unwrap(),
            None => b.body(Body::empty()).unwrap(),
        };
        let res = server::router(st.clone(), Listener::Local).oneshot(req).await.unwrap();
        let status = res.status();
        let ctype = res.headers().get("content-type").map(|v| v.to_str().unwrap().to_string()).unwrap_or_default();
        let bytes = axum::body::to_bytes(res.into_body(), 64 * 1024 * 1024).await.unwrap().to_vec();
        (status, bytes, ctype)
    }

    fn j(b: &[u8]) -> Value {
        serde_json::from_slice(b).unwrap()
    }

    #[tokio::test]
    async fn endpoints() {
        let (root, st) = fixture();

        // maps: merged across folders, flags, authors, pictures
        let (s, b, _) = call(&st, "GET", "/api/maps", None, false).await;
        assert_eq!(s, StatusCode::OK);
        let maps = j(&b);
        let maps = maps.as_array().unwrap();
        assert_eq!(maps.len(), 4, "{maps:?}");
        let by_name = |n: &str| maps.iter().find(|m| m["name"] == n).unwrap().clone();
        assert_eq!(by_name("de_dust2")["source"], "stock");
        assert_eq!(by_name("de_dust2")["thumb"], "lookup", "no picture on disk and not known-missing");
        assert_eq!(by_name("fun_added")["source"], "added");
        assert!(by_name("fun_added").get("mod").is_none());
        let alpha = maps.iter().find(|m| m["name"] == "de_alpha").unwrap();
        assert_eq!(alpha["cat"], "defuse");
        assert_eq!(alpha["nav"], true);
        assert_eq!(alpha["thumb"], "shot");
        assert_eq!(alpha["by"], "Valve");
        assert_eq!(alpha["fav"], false);
        assert_eq!(alpha["recent"], -1);
        assert_eq!(alpha["source"], "stock");
        let beta = maps.iter().find(|m| m["name"] == "aim_beta").unwrap();
        assert_eq!(beta["thumb"], "radar");
        assert_eq!(beta["by"], "Someone");
        assert_eq!(beta["cat"], "arena");
        assert_eq!(beta["title"], "");
        assert_eq!((beta["source"].clone(), beta["mod"].clone(), beta["pack"].clone()), (json!("download"), json!(42), json!(["aim_beta"])));

        // favourites: X-Panel required, names checked
        let flag = json!({"flag": "fav", "map": "de_alpha", "on": true});
        assert_eq!(call(&st, "POST", "/api/mapflag", Some(flag.clone()), false).await.0, StatusCode::FORBIDDEN);
        let (s, b, _) = call(&st, "POST", "/api/mapflag", Some(flag), true).await;
        assert_eq!((s, j(&b)), (StatusCode::OK, json!({"ok": true})));
        let bad = json!({"flag": "fav", "map": "de_x;quit", "on": true});
        assert_eq!(call(&st, "POST", "/api/mapflag", Some(bad), true).await.0, StatusCode::BAD_REQUEST);
        let bad = json!({"flag": "recent", "map": "de_alpha", "on": true});
        assert_eq!(call(&st, "POST", "/api/mapflag", Some(bad), true).await.0, StatusCode::BAD_REQUEST);
        let (_, b, _) = call(&st, "GET", "/api/maps", None, false).await;
        assert_eq!(j(&b).as_array().unwrap().iter().find(|m| m["name"] == "de_alpha").unwrap()["fav"], true);
        assert!(st.game().unwrap().read_list("fav") == vec!["de_alpha".to_string()]);

        // pictures
        let req = Request::builder().uri("/thumb/aim_beta.png").header("host", HOST).body(Body::empty()).unwrap();
        let res = server::router(st.clone(), Listener::Local).oneshot(req).await.unwrap();
        assert_eq!(res.headers()["x-thumb-kind"], "radar");
        let (s, b, ct) = call(&st, "GET", "/thumb/de_alpha.png", None, false).await;
        assert_eq!((s, ct.as_str()), (StatusCode::OK, "image/png"));
        assert_eq!(image::load_from_memory(&b).unwrap().width(), 4);
        let (s, b, _) = call(&st, "GET", "/thumb/aim_beta.png", None, false).await;
        assert_eq!(s, StatusCode::OK);
        let radar = image::load_from_memory(&b).unwrap();
        assert_eq!((radar.width(), radar.height()), (512, 300));
        assert_eq!(call(&st, "GET", "/thumb/de_gamma.png", None, false).await.0, StatusCode::NOT_FOUND);
        assert_eq!(call(&st, "GET", "/thumb/a%20b.png", None, false).await.0, StatusCode::NOT_FOUND);

        // health keeps panel.py's shape
        let (_, b, _) = call(&st, "GET", "/api/health", None, false).await;
        let h = j(&b);
        assert!(h["checks"].as_array().unwrap().iter().all(|c| c.get("name").is_some() && c.get("detail").is_some()));
        assert!(h["fixed"].is_array());

        // map store, without the network
        let (s, b, _) = call(&st, "GET", "/api/gb/browse?cat=1", None, false).await;
        assert_eq!((s, j(&b)), (StatusCode::BAD_REQUEST, json!({"error": "unknown category"})));
        assert_eq!(call(&st, "GET", "/api/gb/browse?page=x", None, false).await.0, StatusCode::BAD_REQUEST);
        let (s, b, _) = call(&st, "GET", "/api/gb/browse?section=skins&cat=5474", None, false).await;
        assert_eq!((s, j(&b)), (StatusCode::BAD_REQUEST, json!({"error": "unknown category"})));
        let (s, b, _) = call(&st, "GET", "/api/gb/browse?section=plugins", None, false).await;
        assert_eq!((s, j(&b)), (StatusCode::BAD_REQUEST, json!({"error": "unknown section"})));
        let (s, b, _) = call(&st, "POST", "/api/gb/install", Some(json!({"mod": 1, "file": 2, "section": "plugins"})), true).await;
        assert_eq!((s, j(&b)["error"].clone()), (StatusCode::BAD_REQUEST, json!("unknown section")));
        let (s, _, _) = call(&st, "POST", "/api/gb/uninstall", Some(json!({"mod": 1, "section": "skins"})), true).await;
        assert_eq!(s, StatusCode::BAD_REQUEST);
        let (s, b, _) = call(&st, "GET", "/api/gb/jobs", None, false).await;
        assert_eq!((s, j(&b)), (StatusCode::OK, json!([])));
        assert_eq!(call(&st, "GET", "/api/gb/mod/12x", None, false).await.0, StatusCode::NOT_FOUND);
        let (s, b, _) = call(&st, "POST", "/api/gb/uninstall", Some(json!({"mod": 999})), true).await;
        assert_eq!(s, StatusCode::BAD_REQUEST);
        assert_eq!(j(&b)["ok"], false);
        assert_eq!(call(&st, "POST", "/api/gb/install", Some(json!({"mod": 1, "file": 2})), false).await.0, StatusCode::FORBIDDEN);
        assert_eq!(call(&st, "POST", "/api/gb/install", Some(json!({"mod": null})), true).await.0, StatusCode::BAD_REQUEST);
        let (s, b, _) = call(&st, "POST", "/api/gb/uninstall", Some(json!({"mod": "42"})), true).await;
        assert_eq!((s, j(&b)), (StatusCode::OK, json!({"ok": true, "maps": ["aim_beta"]})));
        assert!(!root.join("Half-Life/cstrike/maps/aim_beta.bsp").exists());
        let (_, b, _) = call(&st, "GET", "/api/maps", None, false).await;
        assert_eq!(j(&b).as_array().unwrap().len(), 3, "cache refreshed after uninstall");

        // sharing (never touches the running game or playit.gg here)
        let (s, b, _) = call(&st, "GET", "/api/share", None, false).await;
        assert_eq!(s, StatusCode::OK);
        let sh = j(&b);
        for k in ["installed", "connected", "claim", "phase", "error", "address", "account", "name", "password", "method", "tailscale", "port"] {
            assert!(sh.get(k).is_some(), "missing {k}");
        }
        assert_eq!(sh["phase"], "off");
        assert_eq!(sh["port"], 27015);
        let (s, b, _) = call(&st, "POST", "/api/share", Some(json!({"action": "method", "method": "tailscale"})), true).await;
        assert_eq!(s, StatusCode::OK);
        assert_eq!((j(&b)["method"].clone(), j(&b)["ok"].clone()), (json!("tailscale"), json!(true)));
        let (s, b, _) = call(&st, "POST", "/api/share", Some(json!({"action": "server", "name": "x", "password": "no spaces"})), true).await;
        assert_eq!(s, StatusCode::BAD_REQUEST);
        assert!(j(&b)["error"].as_str().unwrap().starts_with("Passwords"));
        assert_eq!(call(&st, "POST", "/api/share", Some(json!({"action": "explode"})), true).await.0, StatusCode::BAD_REQUEST);
        let (s, b, _) = call(&st, "POST", "/api/share", Some(json!({"action": "start"})), true).await;
        assert_eq!((s, j(&b)["error"].clone()), (StatusCode::BAD_REQUEST, json!("Set up playit.gg first.")));
        assert_eq!(call(&st, "POST", "/api/share", Some(json!({"action": "stop"})), false).await.0, StatusCode::FORBIDDEN);

        // commands are checked before anything reaches the game
        let (s, b, _) = call(&st, "POST", "/api/cmd", Some(json!({"cmd": "quit"})), true).await;
        assert_eq!((s, j(&b)["error"].clone()), (StatusCode::BAD_REQUEST, json!("unknown command")));
        let (s, _, _) = call(&st, "POST", "/api/cmd", Some(json!({"cmd": "gm_map", "args": ["de_x;quit"]})), true).await;
        assert_eq!(s, StatusCode::BAD_REQUEST);

        // other hosts are refused (DNS rebinding)
        let req = Request::builder().uri("/api/maps").header("host", "evil.example:8027").body(Body::empty()).unwrap();
        assert_eq!(server::router(st.clone(), Listener::Local).oneshot(req).await.unwrap().status(), StatusCode::FORBIDDEN);

        let _ = std::fs::remove_dir_all(&root);
    }

    /// The map list on this computer's real game, timed: `cargo test -- --ignored real_maps_speed --nocapture`
    #[test]
    #[ignore]
    fn real_maps_speed() {
        let Some(install) = crate::steam::find_installs().into_iter().find(|i| i.games.iter().any(|g| g == "czero")) else { return };
        let app = crate::sys::make_temp_dir(&std::env::temp_dir().join("loner-strike-tests"), "real-").unwrap();
        crate::sys::adopt(&crate::sys::app_dir().join("installed_maps.json"), &app.join("installed_maps.json"));
        let g = crate::game::Game::with_app_dir(&install.hl, "czero", &app);
        let t = std::time::Instant::now();
        let maps = g.maps(true);
        let cold = t.elapsed();
        let t = std::time::Instant::now();
        let again = g.maps(false);
        let cached = t.elapsed();
        let count = |k: &str| maps.iter().filter(|m| m["source"] == k).count();
        eprintln!("{} maps: cold {cold:?}, cached {cached:?}; stock {}, download {}, added {}", maps.len(), count("stock"), count("download"), count("added"));
        assert_eq!(maps.len(), again.len());
        assert!(cached.as_millis() < 500);
        let _ = std::fs::remove_dir_all(&app);
    }
}
