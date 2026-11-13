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
            log: Arc::new(Log::default()),
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
        ctx: Ctx { app: crate::server::router(st, crate::server::Listener::Local), host: HOST.into(), changes: false, confirm: Arc::new(|_| false), log: Arc::new(Log::default()) },
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
    let c = setup_command("http://127.0.0.1:8029/mcp", "TOKEN");
    assert_eq!(c, r#"claude mcp add --transport http loner-strike http://127.0.0.1:8029/mcp --header "Authorization: Bearer TOKEN""#);
    assert!(stdio_command("/Applications/Loner-Strike.app/Contents/MacOS/loner-strike", 8029, "T").ends_with(r#"-- "/Applications/Loner-Strike.app/Contents/MacOS/loner-strike" --mcp-stdio"#));
    assert_eq!(short(&json!({"map": "de_dust2", "x": [1, 2]})), "map: de_dust2, x: [1,2]");
    assert!(short(&json!({"t": "y".repeat(500)})).chars().count() <= 120);
}
