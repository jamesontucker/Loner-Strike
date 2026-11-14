use super::*;
use axum::body::Body;
use axum::extract::Path;
use axum::http::Request;
use tower::ServiceExt;

const HOST: &str = "127.0.0.1:8027";

/// What the stand-in app was asked: "METHOD path body".
type Calls = Arc<Mutex<Vec<String>>>;

/// A stand-in for the app's routes: answers like them and notes every request (and refuses
/// any that didn't come the way the app window's do).
fn fake_app(calls: Calls, dirty: bool, people: usize) -> Router {
    use axum::middleware::{self, Next};
    let c = calls.clone();
    let note = move |req: axum::extract::Request, next: Next| {
        let c = c.clone();
        async move {
            if req.headers().get(header::HOST).and_then(|h| h.to_str().ok()) != Some(HOST) {
                return (StatusCode::FORBIDDEN, "forbidden").into_response();
            }
            if req.method() == axum::http::Method::POST && req.headers().get("x-panel").is_none() {
                return (StatusCode::FORBIDDEN, "no x-panel").into_response();
            }
            let (parts, body) = req.into_parts();
            let bytes = axum::body::to_bytes(body, 1 << 20).await.unwrap();
            let target = parts.headers.get("x-target").and_then(|t| t.to_str().ok()).map(|t| format!(" @{t}")).unwrap_or_default();
            c.lock().unwrap().push(format!("{} {}{} {}", parts.method, parts.uri, target, String::from_utf8_lossy(&bytes)));
            next.run(axum::extract::Request::from_parts(parts, Body::from(bytes))).await
        }
    };
    let players: Vec<Value> = (0..people).map(|i| json!({"name": format!("Player{i}"), "bot": 0})).chain([json!({"name": "Bot", "bot": 1})]).collect();
    Router::new()
        .route("/api/tools/maps", get(|| async { Json(json!({"maps": [{"name": "de_box"}]})) }))
        .route("/api/tools/map/:name/info", get(|Path(n): Path<String>| async move { Json(json!({"name": n, "spawns": 4})) }))
        .route("/api/tools/map/:name/nav", get(|| async {
            Json(json!({"version": 5, "matchesMap": true, "places": ["Mid", "Long"],
                "areas": [{"id": 1, "attr": 1, "nw": [0, 0, 0], "se": [100, 100, 0], "conns": [[2], [], [], []], "place": 1, "hiding": []},
                          {"id": 2, "attr": 0, "nw": [100, 0, 0], "se": [200, 100, 0], "conns": [[], [], [1], []], "place": 0, "hiding": [{"id": 1}]}],
                "ladders": [], "oneWay": 0, "danglingLinks": 0}))
        }))
        .route("/api/tools/editor/doc", get(move || async move { Json(json!({"state": {"name": "arena", "dirty": dirty, "saved": true}, "brushes": [], "entities": [{"id": 1, "classname": "info_player_start"}, {"id": 2, "classname": "light"}]})) }))
        .route("/api/tools/editor/cmd", post(|Json(b): Json<Value>| async move { Json(json!({"created": {"brushes": [7]}, "got": b})) }))
        .route("/api/tools/editor/new", post(|| async { Json(json!({"state": {"name": "Untitled"}})) }))
        .route("/api/tools/editor/save", post(|| async { Json(json!({"saved": true})) }))
        .route("/api/tools/navfix/:name/save", post(|| async { Json(json!({"nav": {"areas": [1, 2, 3]}, "reach": [0, 1, 0], "dirty": false, "problems": []})) }))
        .route("/api/servers", get(|| async {
            Json(json!({"servers": [
                {"id": "mine", "name": "Mine", "state": "running", "host": null, "password": "secret", "dir": "/x"},
                {"id": "far", "name": "Far", "state": "running", "host": "203.0.113.5"},
                {"id": "lent", "name": "Lent", "state": "running", "host": null, "shared": true},
            ]}))
        }))
        .route("/api/servers/:id/:action", post(|| async { Json(json!({"ok": true})) }))
        .route("/api/state", get(move || {
            let players = players.clone();
            async move { Json(json!({"online": true, "map": "de_box", "players": players})) }
        }))
        .route("/api/cmd", post(|| async { Json(json!({"ok": true, "output": []})) }))
        .layer(middleware::from_fn(note))
}

struct T {
    ctx: Ctx,
    calls: Calls,
    asked: Arc<Mutex<Vec<String>>>,
    sessions: rpc::Sessions,
}

fn t(changes: bool, allow: bool, dirty: bool, people: usize) -> T {
    let calls: Calls = Arc::default();
    let asked: Arc<Mutex<Vec<String>>> = Arc::default();
    let a = asked.clone();
    T {
        ctx: Ctx {
            app: fake_app(calls.clone(), dirty, people),
            host: HOST.into(),
            changes,
            confirm: Arc::new(move |text| {
                a.lock().unwrap().push(text);
                allow
            }),
            log: Arc::new(Log::default()), st: None,
        },
        calls,
        asked,
        sessions: rpc::Sessions::default(),
    }
}

fn headers(pairs: &[(&str, &str)]) -> HeaderMap {
    let mut h = HeaderMap::new();
    for (k, v) in pairs {
        h.insert(axum::http::HeaderName::from_bytes(k.as_bytes()).unwrap(), v.parse().unwrap());
    }
    h
}

fn meta() -> Value {
    json!({"io.modelcontextprotocol/protocolVersion": rpc::MODERN, "io.modelcontextprotocol/clientInfo": {"name": "test", "version": "1"}, "io.modelcontextprotocol/clientCapabilities": {}})
}

/// A modern request with its headers mirrored.
async fn modern(t: &T, method: &str, mut params: Value) -> rpc::Reply {
    params["_meta"] = meta();
    let name = params.get("name").or(params.get("uri")).and_then(Value::as_str).map(String::from);
    let mut h = vec![("mcp-protocol-version", rpc::MODERN), ("mcp-method", method)];
    if let Some(n) = &name {
        h.push(("mcp-name", n.as_str()));
    }
    let body = json!({"jsonrpc": "2.0", "id": 1, "method": method, "params": params});
    rpc::handle(&t.ctx, &t.sessions, &headers(&h), body.to_string().as_bytes()).await
}

async fn call_tool(t: &T, name: &str, args: Value) -> Value {
    let r = modern(t, "tools/call", json!({"name": name, "arguments": args})).await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    r.body.unwrap()["result"].clone()
}

fn text(result: &Value) -> String {
    result["content"][0]["text"].as_str().unwrap().to_string()
}

#[tokio::test]
async fn legacy_clients_initialize_and_get_a_session() {
    let t = t(false, false, false, 0);
    let init = json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "c", "version": "1"}}});
    let r = rpc::handle(&t.ctx, &t.sessions, &HeaderMap::new(), init.to_string().as_bytes()).await;
    assert_eq!(r.status, StatusCode::OK);
    let b = r.body.unwrap();
    assert_eq!(b["result"]["protocolVersion"], "2025-06-18", "the client's version when we speak it");
    assert_eq!(b["result"]["serverInfo"]["name"], "loner-strike");
    assert!(b["result"]["capabilities"]["tools"].is_object());
    assert!(b["result"].get("resultType").is_none(), "no modern-only fields for a legacy client");
    let sid = r.session.expect("a session id");

    // a version we don't know: our newest legacy one
    let init2 = json!({"jsonrpc": "2.0", "id": 2, "method": "initialize", "params": {"protocolVersion": "2024-01-01"}});
    let r2 = rpc::handle(&t.ctx, &t.sessions, &HeaderMap::new(), init2.to_string().as_bytes()).await;
    assert_eq!(r2.body.unwrap()["result"]["protocolVersion"], rpc::LEGACY[0]);

    // initialized: a notification, 202 and nothing else
    let n = json!({"jsonrpc": "2.0", "method": "notifications/initialized"});
    let r = rpc::handle(&t.ctx, &t.sessions, &headers(&[("mcp-session-id", &sid)]), n.to_string().as_bytes()).await;
    assert_eq!((r.status, r.body.is_none()), (StatusCode::ACCEPTED, true));

    let list = json!({"jsonrpc": "2.0", "id": 3, "method": "tools/list"});
    let r = rpc::handle(&t.ctx, &t.sessions, &headers(&[("mcp-session-id", &sid)]), list.to_string().as_bytes()).await;
    assert_eq!(r.status, StatusCode::OK);
    assert!(r.body.unwrap()["result"]["tools"].as_array().unwrap().len() > 30);

    // an old session (the app restarted): 404, so the client initializes again
    let r = rpc::handle(&t.ctx, &t.sessions, &headers(&[("mcp-session-id", "nope")]), list.to_string().as_bytes()).await;
    assert_eq!(r.status, StatusCode::NOT_FOUND);
    // neither a session nor a version: a plain 400 a dual-era client falls back on
    let r = rpc::handle(&t.ctx, &t.sessions, &HeaderMap::new(), list.to_string().as_bytes()).await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);
    assert_eq!(r.body.unwrap()["error"]["code"], rpc::INVALID_REQUEST);
}

#[tokio::test]
async fn modern_requests_carry_their_version_and_headers() {
    let t = t(false, false, false, 0);
    let r = modern(&t, "server/discover", json!({})).await;
    let b = r.body.unwrap();
    assert_eq!(b["result"]["supportedVersions"], json!([rpc::MODERN]));
    assert_eq!(b["result"]["resultType"], "complete");
    assert_eq!(b["result"]["_meta"]["io.modelcontextprotocol/serverInfo"]["name"], "loner-strike");

    let r = modern(&t, "tools/list", json!({})).await;
    assert_eq!(r.status, StatusCode::OK);
    assert!(r.session.is_none(), "no sessions in the modern revision");

    let body = |method: &str, params: Value| json!({"jsonrpc": "2.0", "id": 9, "method": method, "params": params}).to_string();
    let mut p = json!({"name": "maps_list", "arguments": {}});
    p["_meta"] = meta();
    // missing, mismatched and badly encoded headers: 400 HeaderMismatch
    for h in [
        headers(&[("mcp-method", "tools/call"), ("mcp-name", "maps_list")]),
        headers(&[("mcp-protocol-version", rpc::MODERN), ("mcp-name", "maps_list")]),
        headers(&[("mcp-protocol-version", rpc::MODERN), ("mcp-method", "tools/call")]),
        headers(&[("mcp-protocol-version", "2025-11-25"), ("mcp-method", "tools/call"), ("mcp-name", "maps_list")]),
        headers(&[("mcp-protocol-version", rpc::MODERN), ("mcp-method", "tools/list"), ("mcp-name", "maps_list")]),
        headers(&[("mcp-protocol-version", rpc::MODERN), ("mcp-method", "tools/call"), ("mcp-name", "map_info")]),
        headers(&[("mcp-protocol-version", rpc::MODERN), ("mcp-method", "tools/call"), ("mcp-name", "=?base64?!!?=")]),
    ] {
        let r = rpc::handle(&t.ctx, &t.sessions, &h, body("tools/call", p.clone()).as_bytes()).await;
        assert_eq!(r.status, StatusCode::BAD_REQUEST, "{h:?}");
        assert_eq!(r.body.unwrap()["error"]["code"], rpc::HEADER_MISMATCH, "{h:?}");
    }
    // the name base64-encoded is the same name
    let h = headers(&[("mcp-protocol-version", rpc::MODERN), ("mcp-method", "tools/call"), ("mcp-name", "=?base64?bWFwc19saXN0?=")]);
    let r = rpc::handle(&t.ctx, &t.sessions, &h, body("tools/call", p.clone()).as_bytes()).await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);

    // a version we don't speak: -32022 with the ones we do
    let mut old = json!({});
    old["_meta"] = json!({"io.modelcontextprotocol/protocolVersion": "1900-01-01"});
    let r = rpc::handle(&t.ctx, &t.sessions, &headers(&[("mcp-protocol-version", "1900-01-01"), ("mcp-method", "tools/list")]), body("tools/list", old).as_bytes()).await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);
    let b = r.body.unwrap();
    assert_eq!(b["error"]["code"], rpc::UNSUPPORTED_VERSION);
    assert!(b["error"]["data"]["supported"].as_array().unwrap().contains(&json!(rpc::MODERN)));

    // an unknown method: 404 + -32601 (so it isn't taken for a legacy server)
    let r = modern(&t, "nope/nothing", json!({})).await;
    assert_eq!(r.status, StatusCode::NOT_FOUND);
    assert_eq!(r.body.unwrap()["error"]["code"], rpc::METHOD_NOT_FOUND);
    // an unknown tool: invalid params
    let r = modern(&t, "tools/call", json!({"name": "rm_rf", "arguments": {}})).await;
    assert_eq!(r.body.unwrap()["error"]["code"], rpc::INVALID_PARAMS);
}

#[tokio::test]
async fn bad_messages_get_jsonrpc_errors() {
    let t = t(false, false, false, 0);
    let h = HeaderMap::new();
    let r = rpc::handle(&t.ctx, &t.sessions, &h, b"{nope").await;
    assert_eq!((r.status, r.body.unwrap()["error"]["code"].clone()), (StatusCode::BAD_REQUEST, json!(rpc::PARSE_ERROR)));
    let r = rpc::handle(&t.ctx, &t.sessions, &h, br#"[{"jsonrpc":"2.0","id":1,"method":"ping"}]"#).await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);
    let r = rpc::handle(&t.ctx, &t.sessions, &h, br#"{"jsonrpc":"1.0","id":1,"method":"ping"}"#).await;
    assert_eq!(r.body.unwrap()["error"]["code"], rpc::INVALID_REQUEST);
    let r = rpc::handle(&t.ctx, &t.sessions, &h, br#"{"jsonrpc":"2.0","id":{"x":1},"method":"ping"}"#).await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);
    let r = rpc::handle(&t.ctx, &t.sessions, &h, br#"{"jsonrpc":"2.0","id":1,"method":"ping","params":[1]}"#).await;
    assert_eq!(r.body.unwrap()["error"]["code"], rpc::INVALID_PARAMS);
}

#[test]
fn only_this_computer_with_the_token_gets_in() {
    let tok = "abcdefghijkmnpqrstuvwxyz23456789";
    let bearer = format!("Bearer {tok}");
    let ok = headers(&[("host", "127.0.0.1:8029"), ("authorization", &bearer)]);
    assert!(check_access(&ok, 8029, tok).is_ok());
    assert!(check_access(&headers(&[("host", "localhost:8029"), ("authorization", &bearer), ("origin", "http://localhost:8029")]), 8029, tok).is_ok());
    // no token, a wrong one, a near miss, another scheme
    for auth in ["", "Bearer ", "Bearer abcdefghijkmnpqrstuvwxyz2345678", "Bearer abcdefghijkmnpqrstuvwxyz23456780", &format!("Basic {tok}")] {
        let h = headers(&[("host", "127.0.0.1:8029"), ("authorization", auth)]);
        assert_eq!(check_access(&h, 8029, tok).unwrap_err().0, StatusCode::UNAUTHORIZED, "{auth}");
    }
    // a web page (DNS rebinding, or another site's script)
    for (host, origin) in [("evil.example:8029", None), ("127.0.0.1:8027", None), ("127.0.0.1:8029", Some("https://evil.example")), ("127.0.0.1:8029", Some("null")), ("127.0.0.1:8029", Some("http://127.0.0.1:8027"))] {
        let mut h = headers(&[("host", host), ("authorization", &bearer)]);
        if let Some(o) = origin {
            h.insert(header::ORIGIN, o.parse().unwrap());
        }
        assert_eq!(check_access(&h, 8029, tok).unwrap_err().0, StatusCode::FORBIDDEN, "{host} {origin:?}");
    }
    // never open with a short (or empty) token
    assert!(check_access(&headers(&[("host", "127.0.0.1:8029"), ("authorization", "Bearer ")]), 8029, "").is_err());
}

#[test]
fn every_tool_is_listed_properly() {
    let tools = catalog::tool_list();
    let mut names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
    for t in &tools {
        let n = t["name"].as_str().unwrap();
        assert!(n.chars().all(|c| c.is_ascii_lowercase() || c == '_') && n.len() <= 64, "{n}");
        assert_eq!(t["inputSchema"]["type"], "object", "{n}");
        assert!(t["inputSchema"]["properties"].is_object(), "{n}");
        assert!(!t["description"].as_str().unwrap().is_empty(), "{n}");
        for r in t["inputSchema"]["required"].as_array().unwrap() {
            assert!(t["inputSchema"]["properties"].get(r.as_str().unwrap()).is_some(), "{n} requires {r}");
        }
    }
    let read: Vec<&str> = tools.iter().filter(|t| t["annotations"]["readOnlyHint"] == true).map(|t| t["name"].as_str().unwrap()).collect();
    for n in ["maps_list", "map_info", "map_doctor", "nav_problems", "replay_summary", "servers_list", "editor_checks"] {
        assert!(read.contains(&n), "{n} is read-only");
    }
    for n in ["editor_add_box", "nav_save", "server_stop", "editor_new", "health_start"] {
        assert!(!read.contains(&n), "{n} changes things");
    }
    for n in ["nav_save", "server_stop", "editor_save", "map_doctor_write_res"] {
        assert!(tools.iter().any(|t| t["name"] == n && t["annotations"]["destructiveHint"] == true), "{n}");
    }
    let count = names.len();
    names.sort();
    names.dedup();
    assert_eq!(names.len(), count, "names are unique");
}

#[tokio::test]
async fn read_only_until_changes_are_allowed() {
    let off = t(false, true, false, 0);
    // reading works
    let r = call_tool(&off, "maps_list", json!({})).await;
    assert_eq!(r["isError"], false);
    assert!(text(&r).contains("de_box"));
    // changing doesn't, and never reaches the app
    for (tool, args) in [
        ("editor_add_box", json!({"min": [0, 0, 0], "max": [64, 64, 64]})),
        ("nav_save", json!({"map": "de_box"})),
        ("server_stop", json!({"server": "mine"})),
        ("editor_command", json!({"op": "delete", "args": {"brushes": [1]}})),
    ] {
        let r = call_tool(&off, tool, args).await;
        assert_eq!(r["isError"], true, "{tool}");
        assert!(text(&r).contains("Allow changes"), "{tool}: {}", text(&r));
    }
    assert!(off.calls.lock().unwrap().iter().all(|c| c.starts_with("GET ")), "{:?}", off.calls.lock().unwrap());
    assert!(off.asked.lock().unwrap().is_empty());
    let log = off.ctx.log.rows();
    assert_eq!(log.len(), 5);
    assert_eq!((log[0]["tool"].clone(), log[0]["outcome"].clone()), (json!("maps_list"), json!("done")));
    assert_eq!((log[1]["tool"].clone(), log[1]["outcome"].clone()), (json!("editor_add_box"), json!("refused")));
    assert!(log[1]["detail"].as_str().unwrap().contains("min: [0,0,0]"));

    // allowed: the box goes to the editor's own command route
    let on = t(true, true, false, 0);
    let r = call_tool(&on, "editor_add_box", json!({"min": [0, 0, 0], "max": [64, 64, 64], "texture": "CRETE1"})).await;
    assert_eq!(r["isError"], false, "{}", text(&r));
    let calls = on.calls.lock().unwrap().clone();
    assert!(calls.iter().any(|c| c.starts_with("POST /api/tools/editor/cmd") && c.contains(r#""op":"add_box""#) && c.contains("CRETE1")), "{calls:?}");
    // bad arguments are the tool's error, not a request
    let r = call_tool(&on, "editor_add_box", json!({"min": [0, 0], "max": [64, 64, 64]})).await;
    assert_eq!(r["isError"], true);
    let r = call_tool(&on, "editor_command", json!({"op": "format_disk"})).await;
    assert_eq!(r["isError"], true);
    let r = call_tool(&on, "map_info", json!({"map": "../../etc/passwd"})).await;
    assert!(text(&r).contains("isn't a map name"));
    let r = call_tool(&on, "health_report", json!({"id": "../x"})).await;
    assert_eq!(r["isError"], true);
}

#[tokio::test]
async fn risky_actions_ask_the_person_first() {
    // nobody on the server: stopped without asking
    let quiet = t(true, false, false, 0);
    let r = call_tool(&quiet, "server_stop", json!({"server": "mine"})).await;
    assert_eq!(r["isError"], false, "{}", text(&r));
    assert!(quiet.asked.lock().unwrap().is_empty());

    // two people on: asked; "Don't allow" stops it
    let busy_no = t(true, false, false, 2);
    let r = call_tool(&busy_no, "server_stop", json!({"server": "mine"})).await;
    assert_eq!(r["isError"], true);
    assert!(text(&r).contains("didn't allow"));
    assert!(busy_no.asked.lock().unwrap()[0].contains("2 people are on it"));
    assert!(!busy_no.calls.lock().unwrap().iter().any(|c| c.contains("/stop")));
    assert_eq!(busy_no.ctx.log.rows().last().unwrap()["outcome"], "refused");

    // "Allow": stopped
    let busy_yes = t(true, true, false, 1);
    let r = call_tool(&busy_yes, "server_stop", json!({"server": "mine"})).await;
    assert_eq!(r["isError"], false);
    assert!(busy_yes.calls.lock().unwrap().iter().any(|c| c.starts_with("POST /api/servers/mine/stop")));

    // only the player's own servers on this computer
    for id in ["far", "lent", "nosuch"] {
        let r = call_tool(&busy_yes, "server_stop", json!({"server": id})).await;
        assert!(text(&r).contains("isn't one of your servers"), "{id}");
    }

    // saving a nav always asks
    let r = call_tool(&t(true, false, false, 0), "nav_save", json!({"map": "de_box"})).await;
    assert_eq!(r["isError"], true);
    let yes = t(true, true, false, 0);
    let r = call_tool(&yes, "nav_save", json!({"map": "de_box"})).await;
    assert_eq!(r["isError"], false);
    let v: Value = serde_json::from_str(&text(&r)).unwrap();
    assert_eq!((v["areas"].clone(), v["unreachable"].clone(), v.get("nav").is_none()), (json!(3), json!(1), true), "the whole nav isn't sent back");

    // a new map over unsaved changes asks; over saved ones it doesn't
    let dirty = t(true, false, true, 0);
    assert_eq!(call_tool(&dirty, "editor_new", json!({})).await["isError"], true);
    assert!(dirty.asked.lock().unwrap()[0].contains("\"arena\""));
    let clean = t(true, false, false, 0);
    assert_eq!(call_tool(&clean, "editor_new", json!({})).await["isError"], false);
    assert!(clean.asked.lock().unwrap().is_empty());

    // saving over the project it came from asks; save as (a dialog) doesn't
    let s = t(true, false, false, 0);
    assert_eq!(call_tool(&s, "editor_save", json!({})).await["isError"], true);
    assert_eq!(call_tool(&s, "editor_save", json!({"as": true})).await["isError"], false);

    // changing the map with people playing asks; the game named "game" has no target
    let g = t(true, true, false, 1);
    let r = call_tool(&g, "server_change_map", json!({"server": "game", "map": "de_dust2", "mode": "gungame"})).await;
    assert_eq!(r["isError"], false, "{}", text(&r));
    assert!(g.asked.lock().unwrap()[0].contains("mode gungame, map de_dust2"));
    assert!(g.calls.lock().unwrap().iter().any(|c| c.starts_with("POST /api/cmd ") && c.contains(r#"["gungame","de_dust2"]"#)), "{:?}", g.calls.lock().unwrap());
    let r = call_tool(&g, "server_set_bots", json!({"server": "mine", "count": 6})).await;
    assert_eq!(r["isError"], false);
    assert!(g.calls.lock().unwrap().iter().any(|c| c.starts_with("POST /api/cmd @mine") && c.contains(r#"["count","6"]"#)));
}

#[tokio::test]
async fn answers_are_trimmed_to_what_an_agent_needs() {
    let t = t(false, false, false, 0);
    let r = call_tool(&t, "servers_list", json!({})).await;
    let v: Value = serde_json::from_str(&text(&r)).unwrap();
    assert_eq!(v["servers"].as_array().unwrap().len(), 1, "own local servers only");
    assert!(!text(&r).contains("secret") && !text(&r).contains("\"dir\""), "no passwords or folders");
    assert_eq!(v["game"]["people"], 0);

    let v: Value = serde_json::from_str(&text(&call_tool(&t, "map_nav_summary", json!({"map": "de_box"})).await)).unwrap();
    assert_eq!((v["areas"].clone(), v["places"]["Mid"].clone(), v["unnamedAreas"].clone(), v["crouchAreas"].clone()), (json!(2), json!(1), json!(1), json!(1)));
    let v: Value = serde_json::from_str(&text(&call_tool(&t, "map_nav_areas", json!({"map": "de_box", "place": "Mid"})).await)).unwrap();
    assert_eq!(v["areas"][0]["id"], 1);
    assert_eq!(v["areas"][0]["links"]["n"], json!([2]));
    let v: Value = serde_json::from_str(&text(&call_tool(&t, "map_nav_areas", json!({"map": "de_box", "near": [150, 50, 0], "radius": 10})).await)).unwrap();
    assert_eq!(v["matched"], 1);
    let v: Value = serde_json::from_str(&text(&call_tool(&t, "editor_list_entities", json!({"classname": "info_"})).await)).unwrap();
    assert_eq!(v["entities"].as_array().unwrap().len(), 1);

    let big = json!({"areas": (0..5000).map(|i| json!({"id": i, "pad": "xxxxxxxxxxxxxxxxxxxxxxxx"})).collect::<Vec<_>>(), "n": 1});
    let small = catalog::trimmed(big);
    assert!(small.to_string().len() <= 48 * 1024);
    assert_eq!(small["n"], 1);
    assert!(small["areasOmitted"].as_u64().unwrap() > 4000);
}

#[test]
fn lsmb_headers_are_read() {
    let head = br#"{"kind":"replay","kills":[1],"sections":[{"name":"t"}]}"#;
    let mut b = b"LSMB".to_vec();
    b.extend(1u32.to_le_bytes());
    b.extend((head.len() as u32).to_le_bytes());
    b.extend(head);
    b.extend([0u8; 16]);
    let v = catalog::lsmb_header(&b).unwrap();
    assert_eq!(v["kind"], "replay");
    assert!(v.get("sections").is_none());
    assert!(catalog::lsmb_header(&b[..20]).is_none(), "cut short");
    let mut lying = b.clone();
    lying[8..12].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(catalog::lsmb_header(&lying).is_none());
}

#[tokio::test]
async fn resources_and_prompts() {
    let t = t(false, false, false, 0);
    let r = modern(&t, "resources/read", json!({"uri": "loner://map/de_box/info"})).await;
    let b = r.body.unwrap();
    assert_eq!(b["result"]["contents"][0]["uri"], "loner://map/de_box/info");
    assert!(b["result"]["contents"][0]["text"].as_str().unwrap().contains("\"spawns\":4"));
    let r = modern(&t, "resources/read", json!({"uri": "file:///etc/passwd"})).await;
    assert!(r.body.unwrap()["error"].is_object());
    assert!(modern(&t, "resources/templates/list", json!({})).await.body.unwrap()["result"]["resourceTemplates"].as_array().unwrap().len() >= 3);

    let r = modern(&t, "prompts/get", json!({"name": "check_map_for_bots", "arguments": {"map": "cs_rats2"}})).await;
    let msg = r.body.unwrap()["result"]["messages"][0]["content"]["text"].as_str().unwrap().to_string();
    assert!(msg.contains("map_doctor cs_rats2"));
    let r = modern(&t, "prompts/get", json!({"name": "check_map_for_bots", "arguments": {}})).await;
    assert!(r.body.unwrap()["error"].is_object(), "the map is required");
    let r = modern(&t, "prompts/get", json!({"name": "make_small_arena", "arguments": {"size": "512"}})).await;
    assert!(r.body.unwrap()["result"]["messages"][0]["content"]["text"].as_str().unwrap().contains("512 by 512"));
}

/// The real app's routes, reached the way the connector reaches them (through the app
/// window's guard): no game here, so the tool says so in the app's own words.
#[tokio::test]
async fn tools_reach_the_real_app_routes() {
    let st: Shared = Arc::new(crate::server::AppState {
        current: RwLock::new(None),
        hosts: RwLock::new(vec![HOST.into()]),
        lan_hosts: RwLock::new(vec![]),
        token: RwLock::new("testtoken0123456789".into()),
        app: OnceLock::new(),
        phone: Mutex::new(crate::app_api::Phone::default()),
    });
    let t = T {
        ctx: Ctx { app: crate::server::router(st, crate::server::Listener::Local), host: HOST.into(), changes: false, confirm: Arc::new(|_| false), log: Arc::new(Log::default()), st: None },
        calls: Arc::default(),
        asked: Arc::default(),
        sessions: rpc::Sessions::default(),
    };
    let r = call_tool(&t, "map_info", json!({"map": "de_dust2"})).await;
    assert_eq!(r["isError"], true);
    assert!(text(&r).contains("Set up the game"), "{}", text(&r));
    let r = call_tool(&t, "health_progress", json!({})).await;
    assert_eq!(r["isError"], false, "{}", text(&r));
}

/// The listener's own route: the token, then the protocol.
#[tokio::test]
async fn the_endpoint_checks_the_token_first() {
    let st: Shared = Arc::new(crate::server::AppState {
        current: RwLock::new(None),
        hosts: RwLock::new(vec![HOST.into()]),
        lan_hosts: RwLock::new(vec![]),
        token: RwLock::new("testtoken0123456789".into()),
        app: OnceLock::new(),
        phone: Mutex::new(crate::app_api::Phone::default()),
    });
    let m = mcp();
    m.enabled.store(true, Ordering::Relaxed);
    let token = m.token.read().unwrap().clone();
    let ep = Arc::new(Endpoint { port: 8029, app: fake_app(Arc::default(), false, 0), host: HOST.into(), st });
    let req = |auth: Option<&str>, method: &str| {
        let mut b = Request::builder().method(method).uri("/mcp").header("host", "127.0.0.1:8029").header("content-type", "application/json");
        if let Some(a) = auth {
            b = b.header("authorization", a);
        }
        b.body(Body::from(r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25"}}"#)).unwrap()
    };
    let res = routes(ep.clone()).oneshot(req(None, "POST")).await.unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    let res = routes(ep.clone()).oneshot(req(Some("Bearer wrongwrongwrongwrongwrongwrong12"), "POST")).await.unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    let res = routes(ep.clone()).oneshot(req(Some(&format!("Bearer {token}")), "POST")).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert!(res.headers().get("mcp-session-id").is_some());
    let res = routes(ep.clone()).oneshot(req(Some(&format!("Bearer {token}")), "GET")).await.unwrap();
    assert_eq!(res.status(), StatusCode::METHOD_NOT_ALLOWED);
}

#[test]
fn the_stdio_bridge_passes_messages_through() {
    // a notification: nothing back; an error without an id gets the request's
    assert!(stdio::answer_for(202, b"", Some(&json!(1))).is_none());
    let a = stdio::answer_for(401, br#"{"jsonrpc":"2.0","error":{"code":-32600,"message":"Wrong or missing token."}}"#, Some(&json!(7))).unwrap();
    assert_eq!(a["id"], 7);
    let a = stdio::answer_for(500, b"oops", Some(&json!("x"))).unwrap();
    assert!(a["error"]["message"].as_str().unwrap().contains("500"));
    // modern requests get their headers mirrored, names outside plain ASCII in base64
    let body = json!({"jsonrpc": "2.0", "id": 1, "method": "resources/read", "params": {"uri": "loner://map/é/info", "_meta": meta()}}).to_string();
    let h = stdio::mirrored(&body);
    assert!(h.contains(&("MCP-Protocol-Version".into(), rpc::MODERN.into())));
    assert!(h.contains(&("Mcp-Method".into(), "resources/read".into())));
    assert!(h.iter().any(|(k, v)| k == "Mcp-Name" && v.starts_with("=?base64?")));
    assert!(stdio::mirrored(r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#).is_empty());
    // HTTP answers: content-length and chunked
    let (s, h, b) = stdio::parse_response(b"HTTP/1.1 200 OK\r\ncontent-length: 2\r\nmcp-session-id: abc\r\n\r\n{}junk").unwrap();
    assert_eq!((s, b.as_slice()), (200, &b"{}"[..]));
    assert!(h.iter().any(|(k, v)| k == "mcp-session-id" && v == "abc"));
    let (_, _, b) = stdio::parse_response(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n3\r\n{\"a\r\n4\r\n\":1}\r\n0\r\n\r\n").unwrap();
    assert_eq!(b, br#"{"a":1}"#);
    assert!(stdio::parse_response(b"garbage").is_err());
}

#[test]
fn setup_commands() {
    let exe = r#"C:\Program Files\Loner-Strike\loner-strike.exe"#;
    let all = conn::snippets(exe, Some("http://127.0.0.1:8029/mcp"), "TOKEN0123456789abcdef");
    let get = |client: &str, kind: &str| all.iter().find(|s| s["client"] == client && s["kind"] == kind).map(|s| s["text"].as_str().unwrap().to_string()).unwrap();
    // stdio setups carry no token or port: they stay valid
    for s in all.iter().filter(|s| s["kind"] == "stdio") {
        let text = s["text"].as_str().unwrap();
        assert!(text.contains("--mcp-stdio") && !text.contains("TOKEN") && !text.contains("8029"), "{text}");
    }
    // (a terminal line: the path as typed, backslashes single - cmd and PowerShell don't unescape)
    assert_eq!(get("claude-code", "stdio"), r#"claude mcp add --scope user --transport stdio loner-strike -- "C:\Program Files\Loner-Strike\loner-strike.exe" --mcp-stdio"#);
    // JSON snippets parse, with the path escaped right
    for client in ["opencode", "cursor", "claude-desktop", "vscode", "other"] {
        let v: Value = serde_json::from_str(&get(client, "stdio")).unwrap_or_else(|e| panic!("{client}: {e}"));
        let entry = v.pointer("/mcp/servers/loner-strike").or(v.pointer("/mcpServers/loner-strike")).or(v.pointer("/servers/loner-strike")).unwrap();
        let cmd = entry["command"].as_str().or(entry["command"][0].as_str()).unwrap();
        assert_eq!(cmd, exe, "{client}");
    }
    let vs: Value = serde_json::from_str(&get("vscode", "http")).unwrap();
    assert_eq!(vs["servers"]["loner-strike"]["type"], "http");
    assert_eq!(vs["servers"]["loner-strike"]["headers"]["Authorization"], "Bearer TOKEN0123456789abcdef");
    let oc: Value = serde_json::from_str(&get("opencode", "http")).unwrap();
    assert_eq!((oc["mcp"]["servers"]["loner-strike"]["type"].clone(), oc["mcp"]["servers"]["loner-strike"]["oauth"].clone()), (json!("remote"), json!(false)));
    let cu: Value = serde_json::from_str(&get("cursor", "http")).unwrap();
    assert_eq!(cu["mcpServers"]["loner-strike"]["url"], "http://127.0.0.1:8029/mcp");
    // Codex: TOML basic strings (same escapes as JSON)
    assert_eq!(get("codex", "stdio"), "[mcp_servers.loner-strike]\ncommand = \"C:\\\\Program Files\\\\Loner-Strike\\\\loner-strike.exe\"\nargs = [\"--mcp-stdio\"]\n");
    assert!(get("codex", "http").contains(r#"http_headers = { "Authorization" = "Bearer TOKEN0123456789abcdef" }"#));
    assert_eq!(get("claude-code", "http"), r#"claude mcp add --scope user --transport http loner-strike http://127.0.0.1:8029/mcp --header "Authorization: Bearer TOKEN0123456789abcdef""#);
    // not listening: stdio setups only
    assert!(conn::snippets("x", None, "t").iter().all(|s| s["kind"] == "stdio"));
    assert_eq!(short(&json!({"map": "de_dust2", "x": [1, 2]})), "map: de_dust2, x: [1,2]");
    assert!(short(&json!({"t": "y".repeat(500)})).chars().count() <= 120);
}

fn scratch(name: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("ls-mcp-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn the_connection_file() {
    let d = scratch("conn");
    let f = d.join("mcp-connection.json");
    assert!(conn::read(&f).is_none());
    conn::write(&f, 8031, "abcdefghijkmnpqrstuvwxyz").unwrap();
    assert_eq!(conn::read(&f), Some((8031, "abcdefghijkmnpqrstuvwxyz".into())));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(std::fs::metadata(&f).unwrap().permissions().mode() & 0o777, 0o600, "this user only");
    }
    // rewritten in one step: no temporary files left beside it
    conn::write(&f, 8032, "zyxwvutsrqpnmkjihgfedcba").unwrap();
    assert_eq!(conn::read(&f).unwrap().0, 8032);
    let names: Vec<String> = std::fs::read_dir(&d).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().to_string()).collect();
    assert_eq!(names, ["mcp-connection.json"]);
    // a damaged or short file is no connection
    std::fs::write(&f, r#"{"port": 8031, "token": "short"}"#).unwrap();
    assert!(conn::read(&f).is_none());
    std::fs::write(&f, "{nope").unwrap();
    assert!(conn::read(&f).is_none());
    conn::write(&f, 8031, "abcdefghijkmnpqrstuvwxyz").unwrap();
    conn::remove(&f);
    assert!(!f.exists());
    conn::remove(&f); // (twice is fine)

    // the environment wins; the file fills in what it doesn't say; nothing at all: a clear "off"
    conn::write(&f, 8031, "filetoken0123456789").unwrap();
    assert_eq!(conn::resolve(None, None, &f), Ok((8031, "filetoken0123456789".into())));
    assert_eq!(conn::resolve(Some("envtoken0123456789".into()), Some("9000".into()), &f), Ok((9000, "envtoken0123456789".into())));
    assert_eq!(conn::resolve(Some("envtoken0123456789".into()), None, &f), Ok((8031, "envtoken0123456789".into())));
    assert_eq!(conn::resolve(None, Some("9000".into()), &f), Ok((9000, "filetoken0123456789".into())));
    assert_eq!(conn::resolve(Some(" ".into()), Some("junk".into()), &f), Ok((8031, "filetoken0123456789".into())), "blank env is unset");
    conn::remove(&f);
    assert_eq!(conn::resolve(None, None, &f), Err(conn::OFF.into()));
    assert_eq!(conn::resolve(Some("envtoken0123456789".into()), None, &f), Ok((PORT, "envtoken0123456789".into())));

    // a kept token: this user only, and only a sane one is used
    let k = d.join("mcp-token");
    conn::keep_token(&k, "keptkeptkeptkept1234").unwrap();
    assert_eq!(conn::kept_token(&k).as_deref(), Some("keptkeptkeptkept1234"));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(std::fs::metadata(&k).unwrap().permissions().mode() & 0o777, 0o600);
    }
    std::fs::write(&k, "short").unwrap();
    assert!(conn::kept_token(&k).is_none());
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn the_bridge_without_the_app_or_after_a_restart() {
    // off / not running: a JSON-RPC error with the request's id, nothing for a notification
    let mut b = stdio::Bridge::default();
    fn never(_: u16, _: &str, _: Option<&str>, _: &str) -> Result<(u16, Vec<(String, String)>, Vec<u8>), String> {
        Err("connection refused".into())
    }
    let a = b.pass(r#"{"jsonrpc":"2.0","id":3,"method":"tools/list"}"#, Err(conn::OFF.into()), never).unwrap();
    assert_eq!(a["id"], 3);
    assert!(a["error"]["message"].as_str().unwrap().contains("Settings > AI agents"));
    assert!(b.pass(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#, Err(conn::OFF.into()), never).is_none());
    let a = b.pass(r#"{"jsonrpc":"2.0","id":4,"method":"ping"}"#, Ok((8029, "t".repeat(20))), never).unwrap();
    assert!(a["error"]["message"].as_str().unwrap().contains("isn't running"));

    // the app restarted: the old session is unknown (404); the bridge greets it again and retries
    static SEEN: Mutex<Vec<String>> = Mutex::new(Vec::new());
    fn app(_: u16, _: &str, session: Option<&str>, body: &str) -> Result<(u16, Vec<(String, String)>, Vec<u8>), String> {
        let method = serde_json::from_str::<Value>(body).unwrap()["method"].as_str().unwrap().to_string();
        SEEN.lock().unwrap().push(format!("{method} {}", session.unwrap_or("-")));
        let ok = |s: &str| Ok((200, vec![("mcp-session-id".to_string(), s.to_string())], br#"{"jsonrpc":"2.0","id":1,"result":{}}"#.to_vec()));
        match (method.as_str(), session) {
            ("initialize", None) => ok(if SEEN.lock().unwrap().len() == 1 { "old" } else { "new" }),
            ("notifications/initialized", _) => Ok((202, vec![], vec![])),
            (_, Some("old")) => Ok((404, vec![], br#"{"jsonrpc":"2.0","id":2,"error":{"code":-32600,"message":"Unknown session: initialize again."}}"#.to_vec())),
            (_, Some("new")) => Ok((200, vec![], br#"{"jsonrpc":"2.0","id":2,"result":{"tools":[]}}"#.to_vec())),
            _ => Ok((400, vec![], vec![])),
        }
    }
    let mut b = stdio::Bridge::default();
    let w = || Ok((8029, "t".repeat(20)));
    b.pass(r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25"}}"#, w(), app).unwrap();
    let a = b.pass(r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#, w(), app).unwrap();
    assert!(a["result"]["tools"].is_array(), "{a}");
    assert_eq!(SEEN.lock().unwrap().as_slice(), ["initialize -", "tools/list old", "initialize -", "notifications/initialized new", "tools/list new"]);
}

/// Serves the connector on 127.0.0.1:8029 with the real game and app routes, without the app
/// window (for trying it with curl, the stdio bridge or an agent; see docs/MCP.md):
/// `LONER_MCP_TOKEN=<16+ chars> [LONER_MCP_CHANGES=1] cargo test serve_mcp_for_try -- --ignored --nocapture`.
/// Questions for the person are answered "Don't allow" (there's no window to ask in).
#[test]
#[ignore]
fn serve_mcp_for_try() {
    let settings = crate::app_api::load_settings();
    let game = crate::servers::game_for("local", || crate::app_api::pick_game(&settings));
    let st: Shared = Arc::new(crate::server::AppState {
        current: RwLock::new(None),
        hosts: RwLock::new(vec![HOST.into()]),
        lan_hosts: RwLock::new(vec![]),
        token: RwLock::new("testtoken0123456789".into()),
        app: OnceLock::new(),
        phone: Mutex::new(crate::app_api::Phone::default()),
    });
    st.set_game(game);
    let m = mcp();
    let _ = m.st.set(st.clone());
    if let Ok(t) = std::env::var("LONER_MCP_TOKEN") {
        *m.token.write().unwrap() = t;
    }
    m.enabled.store(true, Ordering::Relaxed);
    m.changes.store(std::env::var("LONER_MCP_CHANGES").is_ok_and(|v| v == "1"), Ordering::Relaxed);
    switch(true);
    let v = view();
    println!("{}\nchanges: {}\n{}", v["url"], v["changes"], v["command"]);
    let mut shown = 0;
    loop {
        std::thread::sleep(std::time::Duration::from_secs(2));
        let new: Vec<Value> = m.log.rows().into_iter().filter(|r| r["n"].as_u64().unwrap_or(0) > shown).collect();
        for r in new {
            shown = r["n"].as_u64().unwrap_or(0);
            println!("{r}");
        }
    }
}

/// When who's playing, or whether the editor has unsaved work, can't be told, the person is
/// asked (never taken as "nobody" / "nothing to lose").
#[tokio::test]
async fn unknown_means_ask() {
    let with = |app: Router, allow: bool| -> T {
        let asked: Arc<Mutex<Vec<String>>> = Arc::default();
        let a = asked.clone();
        T {
            ctx: Ctx { app, host: HOST.into(), changes: true, confirm: Arc::new(move |t| { a.lock().unwrap().push(t); allow }), log: Arc::new(Log::default()), st: None },
            calls: Arc::default(),
            asked,
            sessions: rpc::Sessions::default(),
        }
    };
    let servers = || get(|| async { Json(json!({"servers": [{"id": "mine", "host": null}]})) });
    let stops: Calls = Arc::default();
    let s2 = stops.clone();
    let stop = post(move |Path((id, action)): Path<(String, String)>| {
        let s2 = s2.clone();
        async move {
            s2.lock().unwrap().push(format!("{id}/{action}"));
            Json(json!({"ok": true}))
        }
    });
    let states: [Router; 3] = [
        // the game didn't take the console password: people may well be on
        Router::new().route("/api/state", get(|| async { Json(json!({"online": false, "reason": "The game didn't accept Loner-Strike's console password."})) })),
        Router::new().route("/api/state", get(|| async { (StatusCode::INTERNAL_SERVER_ERROR, "boom") })),
        Router::new().route("/api/state", get(|| async { Json(json!({"online": true})) })),
    ];
    for state in states {
        let app = state.clone().route("/api/servers", servers()).route("/api/servers/:id/:action", stop.clone()).route("/api/cmd", post(|| async { Json(json!({"ok": true})) }));
        let no = with(app.clone(), false);
        let r = call_tool(&no, "server_stop", json!({"server": "mine"})).await;
        assert_eq!(r["isError"], true, "{}", text(&r));
        assert!(no.asked.lock().unwrap()[0].contains("couldn't tell who's on"));
        let r = call_tool(&no, "server_change_map", json!({"server": "mine", "map": "de_dust2"})).await;
        assert_eq!(r["isError"], true);
        assert!(no.asked.lock().unwrap()[1].contains("couldn't tell who's playing"));
        assert!(stops.lock().unwrap().is_empty(), "nothing stopped without a yes");
        let yes = with(app, true);
        assert_eq!(call_tool(&yes, "server_stop", json!({"server": "mine"})).await["isError"], false);
        assert_eq!(stops.lock().unwrap().pop().as_deref(), Some("mine/stop"));
    }

    // the editor: a doc that can't be read is asked about; nothing open goes ahead
    let new = || post(|| async { Json(json!({"state": {"name": "Untitled"}})) });
    let broken = Router::new().route("/api/tools/editor/doc", get(|| async { Json(json!({"error": "Loner-Strike hit an internal error."})) })).route("/api/tools/editor/new", new());
    let t = with(broken.clone(), false);
    assert_eq!(call_tool(&t, "editor_new", json!({})).await["isError"], true);
    assert!(t.asked.lock().unwrap()[0].contains("couldn't tell whether it has unsaved changes"));
    let t = with(Router::new().route("/api/tools/editor/new", new()), false);
    assert_eq!(call_tool(&t, "editor_new", json!({})).await["isError"], true, "no doc route answer at all: asked too");
    let empty = Router::new().route("/api/tools/editor/doc", get(|| async { Json(json!({"error": "No map is open in the editor. Start a new map or open one."})) })).route("/api/tools/editor/new", new());
    let t = with(empty, false);
    assert_eq!(call_tool(&t, "editor_new", json!({})).await["isError"], false);
    assert!(t.asked.lock().unwrap().is_empty());
}

#[tokio::test]
async fn compiling_and_playtest() {
    let calls: Calls = Arc::default();
    let c = calls.clone();
    let note = move |b: String| {
        c.lock().unwrap().push(b);
        Json(json!({"ok": true}))
    };
    let (n1, n2, n3) = (note.clone(), note.clone(), note.clone());
    let app = Router::new()
        .route("/api/tools/editor/doc", get(|| async { Json(json!({"state": {"name": "arena", "dirty": false}})) }))
        .route("/api/tools/compile/start", post(move |b: String| { let n = n1.clone(); async move { n(format!("start {b}")) } }))
        .route("/api/tools/compile/setup", post(move |b: String| { let n = n2.clone(); async move { n(format!("setup {b}")) } }))
        .route("/api/app/launch", post(move |b: String| { let n = n3.clone(); async move { n(format!("launch {b}")) } }));
    let make = |allow: bool| {
        let asked: Arc<Mutex<Vec<String>>> = Arc::default();
        let a = asked.clone();
        T { ctx: Ctx { app: app.clone(), host: HOST.into(), changes: true, confirm: Arc::new(move |t| { a.lock().unwrap().push(t); allow }), log: Arc::new(Log::default()), st: None }, calls: Arc::default(), asked, sessions: rpc::Sessions::default() }
    };
    let no = make(false);
    // compiling to the work folder doesn't ask; installing into the game, downloading and starting the game do
    assert_eq!(call_tool(&no, "compile_start", json!({"preset": "fast"})).await["isError"], false);
    for (tool, args) in [("compile_start", json!({"install": true})), ("compile_setup", json!({})), ("playtest", json!({"map": "arena", "bots": 4}))] {
        assert_eq!(call_tool(&no, tool, args).await["isError"], true, "{tool}");
    }
    assert_eq!(no.asked.lock().unwrap().len(), 3);
    assert!(no.asked.lock().unwrap()[0].contains("arena.bsp"));
    assert_eq!(calls.lock().unwrap().as_slice(), [r#"start {"preset":"fast"}"#]);
    let yes = make(true);
    assert_eq!(call_tool(&yes, "playtest", json!({"map": "arena", "bots": 4, "mode": "gungame"})).await["isError"], false);
    assert!(calls.lock().unwrap().last().unwrap().contains(r#""map":"arena""#));
    assert_eq!(call_tool(&yes, "compile_start", json!({"name": "../x"})).await["isError"], true);
}

// ---- the whole app: routes, user-only, console, logs, traces ------------------------------

#[test]
fn what_agents_may_do_with_each_route() {
    use routes::{policy, Policy};
    let none = json!({});
    let user_only = |m: &str, p: &str, b: Value| matches!(policy(m, p, &b), Policy::UserOnly(_));
    let asks = |m: &str, p: &str, b: Value| matches!(policy(m, p, &b), Policy::Ask(_));
    // the person's alone, whatever the method or body
    for (m, p, b) in [
        ("GET", "/api/mcp", none.clone()), ("POST", "/api/mcp", json!({"changes": true})), ("POST", "/api/mcp/token", none.clone()),
        ("POST", "/api/app/update/install", none.clone()), ("POST", "/api/app/update/check", none.clone()), ("POST", "/api/app/remove-everything", none.clone()),
        ("POST", "/api/app/restart", none.clone()), ("POST", "/api/setup/uninstall", json!({"hl": "x", "games": ["cstrike"]})),
        ("POST", "/api/setup/install", json!({"hl": "x", "games": ["cstrike"], "components": []})),
        ("POST", "/api/setup/install", json!({"hl": "x", "games": ["cstrike"], "components": ["core"], "repair": true})),
        ("GET", "/api/app/phone", none.clone()), ("POST", "/api/app/phone", json!({"enabled": true})), ("POST", "/api/app/phone/reset", none.clone()),
        ("POST", "/api/control/invite", none.clone()), ("POST", "/api/control/open-port", none.clone()), ("POST", "/api/control/server/abc/invite", none.clone()),
        ("POST", "/api/share", none.clone()), ("POST", "/api/servers/abc/rcon-password", none.clone()), ("POST", "/api/servers/abc/rcon-password/show", none.clone()),
        ("POST", "/api/servers/abc/webstatus/secret", none.clone()), ("POST", "/api/safety/defaults", json!({"ids": []})), ("POST", "/api/safety/clear", none.clone()), ("GET", "/api/nothing/here", none.clone()), ("POST", "/api/servers/abc/hltv/../../mcp", none.clone()),
    ] {
        assert!(user_only(m, p, b.clone()), "{m} {p} {b}");
    }
    // credits: only the person can say permission was given
    for path in ["/api/tools/credits/permission", "/api/tools/credits/manual"] {
        assert!(matches!(routes::policy("POST", path, &none), routes::Policy::UserOnly(_)), "{path}");
    }
    assert_eq!(routes::policy("GET", "/api/tools/credits/map/de_dust2", &none), routes::Policy::Read);
    // asked every time: deleting, removing, kicking, stopping, closing the game, outward
    for (m, p, b) in [
        ("POST", "/api/servers/abc/delete", none.clone()), ("POST", "/api/servers/abc/stop", none.clone()), ("POST", "/api/servers/abc/restart", none.clone()),
        ("POST", "/api/servers/abc/join", json!({"restart": true})), ("POST", "/api/admins/kick", none.clone()), ("POST", "/api/admins/ban", none.clone()),
        ("POST", "/api/gb/uninstall", none.clone()), ("POST", "/api/gb/install", none.clone()), ("POST", "/api/app/launch", json!({"map": "de_dust2"})),
        ("POST", "/api/app/storage/clear", none.clone()), ("POST", "/api/tools/demos/delete", none.clone()), ("POST", "/api/tools/plugins/delete", none.clone()),
        ("POST", "/api/tools/editor/prefabs/delete", none.clone()), ("POST", "/api/tools/community/register", none.clone()), ("POST", "/api/tools/community/report", none.clone()), ("POST", "/api/tools/community/publish/de_x", none.clone()),
        ("POST", "/api/tools/community/uninstall", none.clone()), ("POST", "/api/tools/community/update", none.clone()), ("POST", "/api/tools/community/update-all", none.clone()), ("POST", "/api/tools/community/go-back", none.clone()), ("POST", "/api/tools/community/server/abc", none.clone()), ("POST", "/api/tools/community/publish-mode/lava", none.clone()),
        ("POST", "/api/servers/abc/webstatus", none.clone()), ("POST", "/api/app/direct", none.clone()), ("POST", "/api/safety/restore", none.clone()),
        ("POST", "/api/safety/undo", json!({"from": 1, "to": 2})), ("POST", "/api/setup/install", json!({"components": ["addon-monstermod"]})),
        ("POST", "/api/setup/uninstall", json!({"components": ["addon-monstermod"]})), ("POST", "/api/cmd", json!({"cmd": "gm_player", "args": ["3", "kick"]})),
        ("POST", "/api/sounds/pack", json!({"id": 1, "action": "remove"})), ("POST", "/api/backup/restore", none.clone()), ("POST", "/api/browser/connect", none.clone()),
        ("POST", "/api/tools/spray/use", none.clone()), ("POST", "/api/tools/spray/design/save", none.clone()), ("POST", "/api/tools/spray/design/delete", none.clone()),
    ] {
        assert!(asks(m, p, b.clone()), "{m} {p} {b} -> {:?}", policy(m, p, &b));
    }
    // plain changes and reads
    assert!(matches!(policy("POST", "/api/cmd", &json!({"cmd": "gm_map", "args": ["de_dust2"]})), Policy::Ask(_)), "people may be playing");
    assert_eq!(policy("POST", "/api/cmd", &json!({"cmd": "gm_bots", "args": ["count", "4"]})), Policy::Change);
    assert_eq!(policy("POST", "/api/servers/abc/start", &none), Policy::Change);
    assert_eq!(policy("POST", "/api/servers", &json!({"name": "x"})), Policy::Change);
    assert_eq!(policy("POST", "/api/servers/import", &none), Policy::Change, "the literal route, not /api/servers/:id");
    assert_eq!(policy("POST", "/api/servers/abc/hltv", &none), Policy::Change);
    assert!(matches!(policy("POST", "/api/navs/learn", &none), Policy::Ask(_)), "a hidden server writes the nav into the game");
    assert_eq!(policy("GET", "/api/state", &none), Policy::Read);
    assert_eq!(policy("GET", "/api/servers/abc/console", &none), Policy::Read);
    assert_eq!(policy("GET", "/api/gb/browse", &none), Policy::Read);
    for p in ["/api/safety/files", "/api/safety/version", "/api/safety/changed"] {
        assert_eq!(policy("GET", p, &none), Policy::Read, "{p}");
    }
    // every listed route has a known area and no route is listed twice
    let mut seen = std::collections::HashSet::new();
    for (m, p) in routes::ROUTES {
        assert!(seen.insert((m, p)), "{m} {p} twice");
        assert!(!routes::area_of(p).is_empty(), "{p} has no area");
    }
}

#[test]
fn secrets_never_reach_agents() {
    let mut v = json!({"servers": [{"id": "a", "password": "joinme", "rcon_password": "x", "hasPassword": true, "password_set": true, "name": "Mine"}],
                       "token": "abc", "webhook": "https://discord", "api_key": "k", "steamid": 123, "lines": ["rcon_password \"hunter2\"", "status"], "empty_password": ""});
    routes::redact(&mut v);
    let text = v.to_string();
    for secret in ["joinme", "hunter2", "abc\"", "discord", "\"k\"", "123"] {
        assert!(!text.contains(secret), "{secret} in {text}");
    }
    assert_eq!(v["servers"][0]["hasPassword"], true, "flags stay");
    assert_eq!(v["servers"][0]["name"], "Mine");
    assert_eq!(v["lines"][1], "status");
}

#[test]
fn the_console_allowlist() {
    use debug::{console_policy, Console};
    for line in ["status", "  amxx   list ", "meta list", "ls_mode status", "ls_creature list", "bot_ls_trace", "bot_ls_surf", "bot_quota", "sv_gravity", "mp_timelimit", "cvarlist bot_ls", "ls_creature_max"] {
        assert_eq!(console_policy(line, false), Ok(Console::Read), "{line}");
    }
    // changes off: anything that isn't a read is refused, even settings with a value
    for line in ["bot_ls_trace 1", "bot_kill", "bot_add", "quit", "kick Bob", "changelevel de_dust2", "sv_password", "amxx unload x", "map de_dust2", "exec server.cfg", "bot_nav_analyze", "gm_map de_dust2"] {
        assert!(console_policy(line, false).is_err(), "{line}");
    }
    // never: chaining, the console password, control characters
    for line in ["status; quit", "rcon_password x", "\"rcon_password\" x", "status\nquit", "", &"x".repeat(201)] {
        assert!(console_policy(line, true).is_err(), "{line:?}");
    }
    // changes on: a change goes, but quitting, kicking, banning, the map and exec ask
    assert_eq!(console_policy("bot_ls_trace 1", true), Ok(Console::Change));
    assert_eq!(console_policy("bot_add", true), Ok(Console::Change));
    for line in ["quit", "kick Bob", "banid 0 1", "changelevel de_dust2", "map de_dust2", "exec x.cfg", "sv_password abc", "amx_ban x"] {
        assert!(matches!(console_policy(line, true), Ok(Console::Ask(_))), "{line}");
    }
}

#[tokio::test]
async fn app_call_follows_the_routes_rules() {
    // read-only: a GET goes, a POST doesn't, the person's own never (even with changes)
    let off = t(false, true, false, 0);
    let r = call_tool(&off, "app_call", json!({"method": "GET", "path": "/api/servers"})).await;
    assert_eq!(r["isError"], false, "{}", text(&r));
    assert!(!text(&r).contains("secret"), "passwords are hidden: {}", text(&r));
    let r = call_tool(&off, "app_call", json!({"method": "POST", "path": "/api/cmd", "body": {"cmd": "gm_map", "args": ["de_dust2"]}})).await;
    assert!(text(&r).contains("Allow changes"));
    let on = t(true, false, false, 0);
    for (m, p) in [("POST", "/api/mcp"), ("POST", "/api/app/update/install"), ("POST", "/api/app/phone"), ("POST", "/api/servers/mine/rcon-password/show"), ("GET", "/api/mcp")] {
        let r = call_tool(&on, "app_call", json!({"method": m, "path": p})).await;
        assert_eq!(r["isError"], true, "{p}");
        assert!(text(&r).contains("person's call") || text(&r).contains("off limits"), "{p}: {}", text(&r));
    }
    assert!(on.asked.lock().unwrap().is_empty(), "never even asked");
    // asks, and "Don't allow" means nothing was sent
    let r = call_tool(&on, "app_call", json!({"method": "POST", "path": "/api/servers/mine/delete"})).await;
    assert_eq!(r["isError"], true);
    assert!(on.asked.lock().unwrap()[0].contains("delete this server"));
    assert!(!on.calls.lock().unwrap().iter().any(|c| c.contains("/delete")));
    // a change goes through, aimed at a server with target
    let r = call_tool(&on, "app_call", json!({"method": "POST", "path": "/api/cmd", "body": {"cmd": "gm_bots", "args": ["count", "4"]}, "target": "mine"})).await;
    assert_eq!(r["isError"], false, "{}", text(&r));
    assert!(on.calls.lock().unwrap().iter().any(|c| c.starts_with("POST /api/cmd @mine")));
    // only the person's own servers as targets; no sneaking paths
    let r = call_tool(&on, "app_call", json!({"method": "GET", "path": "/api/state", "target": "far"})).await;
    assert_eq!(r["isError"], true);
    for p in ["/api/../mcp", "/api/state?x=1", "api/state", "/api//state"] {
        assert_eq!(call_tool(&on, "app_call", json!({"method": "GET", "path": p})).await["isError"], true, "{p}");
    }
    // the route list says what each one may do
    let v: Value = serde_json::from_str(&text(&call_tool(&off, "app_routes", json!({"search": "admins kick"})).await)).unwrap();
    assert!(v["routes"].as_array().unwrap().iter().any(|r| r["may"] == "asks the person first"));
    assert!(v["userOnly"].as_array().unwrap().len() >= 4);
}

#[tokio::test]
async fn the_console_through_the_tools() {
    let off = t(false, true, false, 0);
    // a server's console: read-only lines pass even with changes off; others are refused before sending
    let r = call_tool(&off, "console_send", json!({"server": "mine", "command": "bot_ls_trace 1"})).await;
    assert_eq!(r["isError"], true);
    assert!(text(&r).contains("only reading"), "{}", text(&r));
    assert!(!off.calls.lock().unwrap().iter().any(|c| c.contains("console/send")));
    let r = call_tool(&off, "brain_trace", json!({"server": "mine", "on": true})).await;
    assert!(text(&r).contains("Allow changes"));
    assert_eq!(off.ctx.log.rows().last().unwrap()["outcome"], "refused");
}

#[test]
fn log_tails_follow_and_filter() {
    let d = scratch("logs");
    let f = d.join("qconsole.log");
    std::fs::write(&f, "L 11/12/2026 - 10:00:00: LSSURF surf_ski: floor 3\nhello\nL 11/12/2026 - 10:00:01: [gm_components] loaded\nrcon_password \"secret1\"\n").unwrap();
    let all = |_: &str| true;
    let v = debug::tail(&f, None, 100, &all).unwrap();
    assert_eq!(v["lines"].as_array().unwrap().len(), 4);
    assert!(!v.to_string().contains("secret1"), "passwords hidden");
    let cursor = v["cursor"].as_str().unwrap().to_string();
    // nothing new yet; then two new lines, only those
    let v = debug::tail(&f, Some(&cursor), 100, &all).unwrap();
    assert!(v["lines"].as_array().unwrap().is_empty());
    std::fs::OpenOptions::new().append(true).open(&f).unwrap().write_all(b"one\nLSSURF two\n").unwrap();
    let v = debug::tail(&f, Some(&cursor), 100, &all).unwrap();
    assert_eq!(v["lines"], json!(["one", "LSSURF two"]));
    // paging: one at a time
    let v1 = debug::tail(&f, Some(&cursor), 1, &all).unwrap();
    assert_eq!((v1["lines"].clone(), v1["more"].clone()), (json!(["one"]), json!(true)));
    let v2 = debug::tail(&f, v1["cursor"].as_str(), 1, &all).unwrap();
    assert_eq!(v2["lines"], json!(["LSSURF two"]));
    // a file that started again (shorter) resets
    std::fs::write(&f, "new\n").unwrap();
    let v = debug::tail(&f, v2["cursor"].as_str(), 100, &all).unwrap();
    assert_eq!((v["lines"].clone(), v["reset"].clone()), (json!(["new"]), json!(true)));
    // prefix filters look past the log's date
    std::fs::write(&f, "L 11/12/2026 - 10:00:00: LSSURF a\nLSSURF b\nother\n").unwrap();
    let keep = |l: &str| {
        let body = l.split_once(": ").filter(|(d, _)| d.starts_with('L') && d.len() < 30).map(|(_, b)| b).unwrap_or(l);
        body.starts_with("LSSURF")
    };
    assert_eq!(debug::tail(&f, None, 100, &keep).unwrap()["lines"].as_array().unwrap().len(), 2);
    use std::io::Write;
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn brain_traces_read_as_columns() {
    let d = scratch("trace");
    let f = d.join("lstrace.csv");
    std::fs::write(&f, "#map,cs_rats2\n#cols,time,id,team,state,task\n10.5,3,1,Hunt,Patrol\n10.5,4,2,Idle,Camp\n#mark,11.0,cs_rats2,before\n11.0,3,1,Attack,Fight\n").unwrap();
    let v = debug::trace_rows(&f, None, 100, Some(3), None, None).unwrap();
    let rows = v["rows"].as_array().unwrap();
    assert_eq!(rows.len(), 3, "{v}"); // bot 3's two rows and the mark
    assert_eq!(rows[0]["task"], "Patrol");
    assert_eq!(rows[0]["time"], 10.5);
    let v = debug::trace_rows(&f, None, 100, None, Some(11.0), None).unwrap();
    assert!(v["rows"].as_array().unwrap().iter().all(|r| r.is_string() || r["time"] == 11.0), "{v}");
    // a recording's trace, one bot in a window, every 2nd row
    let rec = json!({"columns": ["t", "id", "task"], "bots": [{"id": 3, "rows": [[1.0, 3, "a"], [2.0, 3, "b"], [3.0, 3, "c"], [4.0, 3, "d"]]}, {"id": 4, "rows": [[1.0, 4, "x"]]}],
                     "why": [{"t": 2.0, "id": 3, "why": "heard"}, {"t": 2.0, "id": 4, "why": "x"}], "marks": []});
    let w = debug::trace_window(&rec, &json!({"bot": 3, "from": 2.0, "every": 2}));
    assert_eq!(w["bots"].as_array().unwrap().len(), 1);
    assert_eq!(w["bots"][0]["rows"], json!([{"t": 2.0, "id": 3, "task": "b"}, {"t": 4.0, "id": 3, "task": "d"}]));
    assert_eq!(w["why"].as_array().unwrap().len(), 1);
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn docs_come_in_pages() {
    let p = debug::doc_page("gamemodes-plan", 0).unwrap();
    let next = p["next"].as_u64().expect("the plan is longer than a page");
    let p2 = debug::doc_page("gamemodes-plan", next as usize).unwrap();
    assert!(p2["text"].as_str().unwrap().len() > 100);
    assert!(debug::doc_page("components", 0).unwrap()["text"].as_str().unwrap().contains("ls_"));
    assert!(debug::doc_page("../secrets", 0).is_none());
    assert!(serde_json::from_str::<Value>(more::STUDIO_NODES).is_ok());
}

/// Every route in the source is listed in routes.rs (or deliberately left out): run it after
/// adding routes, `cargo test mcp_route_coverage -- --ignored --nocapture`.
#[test]
#[ignore]
fn mcp_route_coverage() {
    let mut missing = vec![];
    let re = regex::Regex::new(r#"\.route\(\s*"([^"]+)",\s*((?:[\w:]+\(\w+\)\.?)+)"#).unwrap();
    fn walk(d: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        for e in std::fs::read_dir(d).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(&p, out);
            } else if p.extension().is_some_and(|x| x == "rs") {
                out.push(p);
            }
        }
    }
    let mut files = vec![];
    walk(std::path::Path::new("src"), &mut files);
    for f in files.iter().filter(|f| !f.to_string_lossy().contains("/mcp/")) {
        let t = std::fs::read_to_string(f).unwrap();
        let t = t.split("#[cfg(test)]").next().unwrap();
        for c in re.captures_iter(t) {
            for m in ["get", "post"] {
                if c[2].contains(&format!("{m}(")) && !routes::ROUTES.iter().any(|(rm, rp)| rm.eq_ignore_ascii_case(m) && *rp == &c[1]) {
                    missing.push(format!("{} {} ({})", m.to_uppercase(), &c[1], f.display()));
                }
            }
        }
    }
    println!("{}", missing.join("\n"));
    assert!(missing.is_empty(), "{} routes aren't listed in mcp/routes.rs", missing.len());
}

// ---- no way round the rules: raw console routes and path tricks -------------------------

#[test]
fn raw_console_routes_follow_the_console_rules() {
    use routes::{policy, Policy};
    let send = |line: &str| policy("POST", "/api/servers/abc/console/send", &json!({"command": line}));
    // the console password and chains: never, even with Allow changes
    for line in ["rcon_password x", "\"RCON_PASSWORD\" x", "rcon status", "status; quit", "bot_add;rcon_password x", "status\nquit", ""] {
        assert!(matches!(send(line), Policy::UserOnly(_)), "{line:?} -> {:?}", send(line));
    }
    // risky lines ask, exactly as console_send asks
    for line in ["quit", "exec server.cfg", "kick Bob", "changelevel de_dust2", "sv_password abc", "amx_ban x", "sv_downloadurl http://x"] {
        assert!(matches!(send(line), Policy::Ask(_)), "{line} -> {:?}", send(line));
    }
    assert_eq!(send("status"), Policy::Read);
    assert_eq!(send("bot_ls_trace 1"), Policy::Change);
    // the same verdict as console_send for every line
    for line in ["status", "bot_kick", "quit", "rcon_password x", "meta list", "exec x", "a;b", "mp_timelimit 5"] {
        let typed = debug::console_policy(line, true);
        let routed = send(line);
        let same = match (&typed, &routed) {
            (Err(_), Policy::UserOnly(_)) | (Ok(debug::Console::Read), Policy::Read) | (Ok(debug::Console::Change), Policy::Change) | (Ok(debug::Console::Ask(_)), Policy::Ask(_)) => true,
            _ => false,
        };
        assert!(same, "{line}: console_send {typed:?} but app_call {routed:?}");
    }
    // a setting written into a mode's config (which the game runs as console lines)
    let ms = |cvar: &str, value: &str| policy("POST", "/api/app/mode-settings", &json!({"mode": "dm", "cvar": cvar, "value": value}));
    assert!(matches!(ms("rcon_password", "x"), Policy::UserOnly(_)));
    assert!(matches!(ms("exec", "x.cfg"), Policy::Ask(_)));
    assert!(matches!(ms("sv_password", "x"), Policy::Ask(_)));
    assert!(matches!(ms("mp_timelimit 5;quit", "1"), Policy::UserOnly(_)));
    assert_eq!(ms("mp_timelimit", "5"), Policy::Change);
    // a server's start options run anything: the person's alone; join password and listing ask
    assert!(matches!(policy("POST", "/api/servers/abc", &json!({"extra_args": "+rcon_password x"})), Policy::UserOnly(_)));
    assert!(matches!(policy("POST", "/api/servers", &json!({"name": "x", "extra_args": "+exec x"})), Policy::UserOnly(_)));
    assert!(matches!(policy("POST", "/api/servers/abc", &json!({"password": "x"})), Policy::Ask(_)));
    assert!(matches!(policy("POST", "/api/servers/abc", &json!({"visibility": "listed"})), Policy::Ask(_)));
    assert_eq!(policy("POST", "/api/servers/abc", &json!({"maxplayers": 12})), Policy::Change);
    // the Studio debugger takes typed fields only (no command text): a plain change
    assert_eq!(policy("POST", "/api/tools/studio/debug/command", &json!({"mode": "x", "resume": true})), Policy::Change);
}

#[test]
fn path_tricks_are_refused() {
    use routes::{canonical, matches, policy, Policy};
    // ":x" never matches nothing, "." or ".."
    assert!(!matches("/api/servers/:id/start", "/api/servers/../start"));
    assert!(!matches("/api/servers/:id/start", "/api/servers/./start"));
    assert!(!matches("/api/servers/:id/start", "/api/servers//start"));
    assert!(matches("/api/servers/:id/start", "/api/servers/abc/start"));
    for p in [
        "/api/servers/../mcp", "/api/servers/./x/start", "/api/servers/abc/../../mcp/token", "/api/./state", "/api/state/", "/api//state", "//api/state",
        "/api/servers/%2e%2e/mcp", "/api/servers/%2E%2E/start", "/api/servers/abc%2fstart", "/api/servers/abc%2Fstart", "/api/servers/abc%5cstart", "/api/servers/abc%5Cstart",
        "/api/servers/abc\\..\\mcp", "/api/state?x=1", "/api/state#x", "/api/st ate", "/api/stäte", "api/state", "", "/", "/api/state\n", "/api/servers/abc/start?/../../mcp",
    ] {
        assert!(canonical(p).is_err(), "{p:?} passed");
        assert!(matches!(policy("POST", p, &json!({})), Policy::UserOnly(_)), "{p:?}");
        assert!(matches!(policy("GET", p, &json!({})), Policy::UserOnly(_)), "{p:?}");
    }
    assert_eq!(canonical("/api/servers/abc/start"), Ok("/api/servers/abc/start"));
    // queries: GETs only (and the screenshot route)
    assert!(routes::query_allowed("GET", "/api/state"));
    assert!(!routes::query_allowed("POST", "/api/servers/:id/:action"));
    assert!(routes::query_allowed("POST", "/api/tools/screenshot"));
}

#[tokio::test]
async fn app_call_sends_exactly_what_it_checked() {
    let on = t(true, true, false, 0);
    for p in ["/api/servers/%2e%2e/mcp", "/api/servers/mine/..", "/api/servers/mine\\start", "/api/servers/./start", "/api/servers/mine/start/"] {
        let r = call_tool(&on, "app_call", json!({"method": "POST", "path": p})).await;
        assert_eq!(r["isError"], true, "{p}");
    }
    // a query on a POST is refused; on a GET it goes, encoded, after the checked path
    let r = call_tool(&on, "app_call", json!({"method": "POST", "path": "/api/servers/mine/start", "query": {"x": "1"}})).await;
    assert_eq!(r["isError"], true);
    let _ = call_tool(&on, "app_call", json!({"method": "GET", "path": "/api/state", "query": {"a": "../../mcp?b=#"}})).await;
    // the raw console route through app_call: the password never, quitting asks
    let r = call_tool(&on, "app_call", json!({"method": "POST", "path": "/api/servers/mine/console/send", "body": {"command": "rcon_password x"}})).await;
    assert_eq!(r["isError"], true);
    assert!(on.asked.lock().unwrap().is_empty());
    let deny = t(true, false, false, 0);
    let r = call_tool(&deny, "app_call", json!({"method": "POST", "path": "/api/servers/mine/console/send", "body": {"command": "quit"}})).await;
    assert_eq!(r["isError"], true);
    assert_eq!(deny.asked.lock().unwrap().len(), 1);
    let calls = on.calls.lock().unwrap().clone();
    let paths: Vec<&str> = calls.iter().map(|c| c.split([' ', '?']).nth(1).unwrap_or("")).collect();
    assert!(!paths.iter().any(|p| p.contains("console/send") || p.contains("mcp") || p.contains('%')), "{calls:?}");
    assert!(calls.iter().any(|c| c.starts_with("GET /api/state?a=..%2F..%2Fmcp%3Fb%3D%23")), "{calls:?}");
    deny.calls.lock().unwrap().iter().for_each(|c| assert!(!c.contains("console/send"), "{c}"));
}

/// No route is weaker through app_call than through the typed tool that uses it: every typed
/// tool is run (changes on, the person saying yes) against a stand-in app that notes every
/// request, and each POST it makes is classified by routes::policy, as app_call would.
#[tokio::test]
async fn typed_tools_are_never_stronger_than_app_call() {
    use axum::middleware::{self, Next};
    use routes::Policy;
    let calls: Calls = Arc::default();
    let c = calls.clone();
    let note = move |req: axum::extract::Request, next: Next| {
        let c = c.clone();
        async move {
            let (parts, body) = req.into_parts();
            let bytes = axum::body::to_bytes(body, 1 << 20).await.unwrap();
            c.lock().unwrap().push(format!("{} {} {}", parts.method, parts.uri, String::from_utf8_lossy(&bytes)));
            next.run(axum::extract::Request::from_parts(parts, Body::from(bytes))).await
        }
    };
    let app = Router::new()
        .route("/api/servers", get(|| async { Json(json!({"servers": [{"id": "mine", "name": "Mine", "state": "running", "host": null}]})) }))
        .fallback(|| async { Json(json!({"ok": true, "id": "x", "events": [], "lines": [], "items": []})) })
        .layer(middleware::from_fn(note));
    let a2 = calls.clone();
    let ctx = Ctx {
        app,
        host: HOST.into(),
        changes: true,
        confirm: Arc::new(move |_| {
            a2.lock().unwrap().push("ASK".into());
            true
        }),
        log: Arc::new(Log::default()),
        st: None,
    };
    // plausible arguments from each tool's schema
    fn sample(name: &str, p: &Value) -> Value {
        if let Some(e) = p["enum"].as_array().and_then(|e| e.first()) {
            return e.clone();
        }
        match (name, p["type"].as_str().unwrap_or("string")) {
            ("map", _) => json!("de_dust2"),
            ("server" | "target", _) => json!("mine"),
            ("command", _) => json!("bot_ls_trace 1"),
            ("cvar" | "name" | "setting", _) => json!("bot_ls_trace"),
            (_, "integer" | "number") => json!(1),
            (_, "boolean") => json!(true),
            (_, "array") => json!([]),
            (_, "object") => json!({}),
            _ => json!("x"),
        }
    }
    let mut checked = std::collections::BTreeSet::new();
    let mut weaker = vec![];
    for spec in catalog::specs() {
        if ["app_call", "app_routes"].contains(&spec.name) {
            continue;
        }
        let schema = (spec.schema)();
        let mut args = serde_json::Map::new();
        for (k, p) in schema["properties"].as_object().into_iter().flatten() {
            args.insert(k.clone(), sample(k, p));
        }
        calls.lock().unwrap().clear();
        let _ = catalog::call_tool(&ctx, spec.name, Value::Object(args)).await;
        let mut asked = false;
        for c in calls.lock().unwrap().iter() {
            if c == "ASK" {
                asked = true;
                continue;
            }
            let mut it = c.splitn(3, ' ');
            let (m, uri, body) = (it.next().unwrap(), it.next().unwrap(), it.next().unwrap_or(""));
            if m != "POST" {
                continue;
            }
            let path = uri.split('?').next().unwrap();
            let body: Value = serde_json::from_str(body).unwrap_or(json!({}));
            let p = routes::policy(m, path, &body);
            checked.insert(format!("{} -> {path}", spec.name));
            let bad = match &p {
                Policy::UserOnly(_) => true,
                Policy::Read | Policy::Change => asked,
                Policy::Ask(_) => false,
            };
            if bad {
                weaker.push(format!("{} (asked: {asked}) POST {path} {body} -> app_call {p:?}", spec.name));
            }
        }
    }
    let unreached: Vec<&str> = catalog::specs().iter().filter(|s| s.kind == catalog::Kind::Risky && !checked.iter().any(|c| c.starts_with(&format!("{} ->", s.name)))).map(|s| s.name).collect();
    println!("risky tools not reached: {unreached:?}");
    assert!(weaker.is_empty(), "app_call is weaker than the typed tool for:\n{}", weaker.join("\n"));
    assert!(checked.len() >= 30, "only {} tool routes were reached: {checked:?}", checked.len());
}


/// Every route has a policy someone chose: GETs read, and every POST is caught by a rule or is
/// on the reviewed list of plain changes (a new route is asked about until it's reviewed).
#[test]
fn every_route_has_an_explicit_policy() {
    let mut unreviewed = vec![];
    for (m, p) in routes::ROUTES {
        let path = p.split('/').map(|x| if x.starts_with(':') { "x1" } else { x }).collect::<Vec<_>>().join("/");
        if let routes::Policy::Ask(w) = routes::policy(m, &path, &json!({})) {
            if w.contains("not reviewed") {
                unreviewed.push(format!("{m} {p}"));
            }
        }
    }
    assert!(unreviewed.is_empty(), "routes with no explicit policy (add a rule or list them in routes::CHANGES):\n{}", unreviewed.join("\n"));
    // the reviewed list names real POST routes only
    for c in routes::CHANGES {
        assert!(routes::ROUTES.contains(&("POST", c)), "{c} isn't a POST route");
    }
    // the ones the review named
    let none = json!({});
    assert!(matches!(routes::policy("POST", "/api/tools/editor/prefabs/update", &none), routes::Policy::UserOnly(_)));
    for p in ["/api/tools/editor/assets/find", "/api/tools/editor/assets/copy", "/api/tools/studio/compile", "/api/tools/doctor/de_x/copy", "/api/tools/overview/de_x/install", "/api/tools/editor/project/action", "/api/tools/health/start"] {
        assert!(matches!(routes::policy("POST", p, &none), routes::Policy::Ask(_)), "{p}");
    }
    assert_eq!(routes::policy("POST", "/api/tools/editor/assets/copy", &json!({"to": "project"})), routes::Policy::Change);
}
