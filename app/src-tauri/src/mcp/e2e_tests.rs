//! End to end, as an agent works: JSON-RPC `tools/call` requests through the connector into
//! the map editor's real routes (its own editor in a temp folder, no game: nothing reaches the
//! person's game or app files) and the Game Mode Studio's own checks and plugin generator.
//! The two compilers are stand-ins (SDHLT and amxxpc need the game; the real map compile has
//! its opt-in test in tools/compile.rs, the Studio's in tools/studio): they answer the way
//! the real routes do, the Studio's from its real checks and generator.

use super::*;
use crate::server::AppState;
use crate::tools::studio::{gen, graph};
use axum::http::HeaderMap;
use std::sync::RwLock;

fn state() -> Shared {
    Arc::new(AppState {
        current: RwLock::new(None),
        hosts: RwLock::new(vec!["127.0.0.1:8027".into()]),
        lan_hosts: RwLock::new(vec![]),
        token: RwLock::new("testtoken0123456789".into()),
        app: OnceLock::new(),
        phone: Mutex::new(crate::app_api::Phone::default()),
    })
}

type Calls = Arc<Mutex<Vec<String>>>;

/// The compilers' stand-ins (same paths and answers as tools/compile.rs and tools/studio).
fn stand_ins(calls: Calls) -> Router<Shared> {
    let (c1, c2) = (calls.clone(), calls.clone());
    Router::new()
        .route("/api/tools/compile/start", post(move |Json(b): Json<Value>| {
            let c = c1.clone();
            async move {
                c.lock().unwrap().push(format!("compile {b}"));
                Json(json!({"ok": true, "job": 1, "map": "ls_agent_room"}))
            }
        }))
        .route("/api/tools/compile/progress", get(|| async {
            Json(json!({"job": {"id": 1, "kind": "compile", "running": false, "stage": "done", "label": "Done", "fraction": 1.0, "problems": [], "leak": null,
                "result": {"bsp": "ls_agent_room.bsp", "installed": false}, "error": null, "stopped": false}, "lines": ["> sdHLCSG", "> sdHLBSP", "> sdHLRAD"], "next": 3}))
        }))
        .route("/api/tools/studio/validate", post(|Json(b): Json<Value>| async move {
            match graph::from_value(b["graph"].clone()) {
                Ok(g) => Json(json!({"problems": graph::validate(&g)})),
                Err(e) => Json(json!({"error": e})),
            }
        }))
        .route("/api/tools/studio/generate", post(|Json(b): Json<Value>| async move {
            let g = match graph::from_value(b["graph"].clone()) {
                Ok(g) => g,
                Err(e) => return Json(json!({"error": e})),
            };
            let problems = graph::validate(&g);
            if graph::has_errors(&problems) {
                return Json(json!({"problems": problems}));
            }
            let out = gen::generate(&g, false);
            Json(json!({"problems": problems, "source": out.source, "spans": out.spans, "plugin": gen::plugin_name(&g.mode.id)}))
        }))
        .route("/api/tools/studio/compile", post(move |Json(b): Json<Value>| {
            let c = c2.clone();
            async move {
                // the real route checks, generates, runs amxxpc and installs; here: checks and
                // generates, and says what it would install
                let g = match graph::from_value(b["graph"].clone()) {
                    Ok(g) => g,
                    Err(e) => return Json(json!({"error": e})),
                };
                let problems = graph::validate(&g);
                if graph::has_errors(&problems) {
                    return Json(json!({"ok": false, "problems": problems}));
                }
                let out = gen::generate(&g, false);
                c.lock().unwrap().push(format!("studio compile {} ({} lines)", g.mode.id, out.source.lines().count()));
                Json(json!({"ok": true, "problems": problems, "plugin": gen::plugin_name(&g.mode.id), "installedAs": g.mode.id, "output": "stand-in: not installed", "debug": false}))
            }
        }))
        .route("/api/tools/studio/graph/:id", get(|| async { Json(json!({"error": "There's no graph with that id."})) }))
        .route("/api/tools/creatures", get(|| async { Json(json!({"types": [], "templates": []})) }))
}

struct Agent {
    ctx: Ctx,
    sessions: rpc::Sessions,
    asked: Arc<Mutex<Vec<String>>>,
    calls: Calls,
    dir: std::path::PathBuf,
}

impl Agent {
    fn new() -> Agent {
        let dir = std::env::temp_dir().join(format!("ls-mcp-e2e-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let calls: Calls = Arc::default();
        let app = crate::tools::editor::routes_in(dir.clone()).merge(stand_ins(calls.clone())).with_state(state());
        let asked: Arc<Mutex<Vec<String>>> = Arc::default();
        let a = asked.clone();
        Agent {
            ctx: Ctx { app, host: "127.0.0.1:8027".into(), changes: true, confirm: Arc::new(move |t| {
                a.lock().unwrap().push(t);
                true
            }), log: Arc::new(Log::default()), st: None },
            sessions: rpc::Sessions::default(),
            asked,
            calls,
            dir,
        }
    }

    /// One tools/call over JSON-RPC (the modern era, headers mirrored); the tool's JSON answer.
    async fn call(&self, name: &str, args: Value) -> Value {
        let meta = json!({"io.modelcontextprotocol/protocolVersion": rpc::MODERN, "io.modelcontextprotocol/clientInfo": {"name": "e2e", "version": "1"}, "io.modelcontextprotocol/clientCapabilities": {}});
        let body = json!({"jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": {"name": name, "arguments": args, "_meta": meta}});
        let mut h = HeaderMap::new();
        h.insert("mcp-protocol-version", rpc::MODERN.parse().unwrap());
        h.insert("mcp-method", "tools/call".parse().unwrap());
        h.insert("mcp-name", name.parse().unwrap());
        let r = rpc::handle(&self.ctx, &self.sessions, &h, body.to_string().as_bytes()).await;
        assert_eq!(r.status, StatusCode::OK, "{name}: {:?}", r.body);
        let res = r.body.unwrap()["result"].clone();
        let text = res["content"][0]["text"].as_str().unwrap_or("").to_string();
        assert_ne!(res["isError"], true, "{name} {args}: {text}");
        serde_json::from_str(&text).unwrap_or(Value::String(text))
    }
}

impl Drop for Agent {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn ids(v: &Value) -> Vec<u64> {
    v.as_array().into_iter().flatten().filter_map(Value::as_u64).collect()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_agent_builds_checks_compiles_a_map_and_makes_a_mode() {
    let ag = Agent::new();

    // ---- 1. a small map: a room, hollowed, a light, spawns, a texture lined up --------------
    ag.call("editor_new", json!({})).await;
    let doc = ag.call("editor_state", json!({"limit": 500})).await;
    let brushes: Vec<u64> = doc["brushes"].as_array().unwrap().iter().filter_map(|b| b["id"].as_u64()).collect();
    let entities: Vec<u64> = doc["entities"].as_array().unwrap().iter().filter_map(|e| e["id"].as_u64()).collect();
    ag.call("editor_delete", json!({"brushes": brushes, "entities": entities})).await;
    let room = ag.call("editor_add_shape", json!({"shape": "box", "min": [-256, -256, 0], "max": [256, 256, 192], "texture": "BRICK01"})).await;
    let room_id = ids(&room["created"]["brushes"])[0];
    let walls = ag.call("editor_hollow", json!({"brushes": [room_id], "thickness": 16})).await;
    let walls = ids(&walls["created"]["brushes"]);
    assert_eq!(walls.len(), 6, "a hollow box is six walls: {walls:?}");
    // the floor: the wall whose top is lowest; its face that looks up
    let faces = ag.call("editor_faces", json!({"brushes": walls})).await;
    let mut floor_face = None;
    let state = ag.call("editor_state", json!({"limit": 500})).await;
    for b in state["brushes"].as_array().unwrap() {
        if b["max"][2].as_f64() == Some(16.0) {
            let id = b["id"].as_u64().unwrap();
            let f = faces["brushes"].as_array().unwrap().iter().find(|x| x["brush"] == id).unwrap();
            let up = f["faces"].as_array().unwrap().iter().find(|x| x["facing"] == "up").unwrap()["face"].as_u64().unwrap();
            floor_face = Some((id, up));
        }
    }
    let (floor_id, up) = floor_face.expect("the floor slab is 0..16 high");
    ag.call("editor_set_texture", json!({"faces": [[floor_id, up]], "texture": "FLOOR01"})).await;
    ag.call("editor_align_texture", json!({"faces": [[floor_id, up]], "action": "set", "scale_u": 0.5, "scale_v": 0.5, "rotation": 0})).await;
    ag.call("editor_align_texture", json!({"faces": [[floor_id, up]], "action": "justify", "mode": "center"})).await;
    let after = ag.call("editor_faces", json!({"brushes": [floor_id]})).await;
    let top = after["brushes"][0]["faces"].as_array().unwrap().iter().find(|f| f["face"] == up).unwrap().clone();
    assert_eq!(top["texture"], "FLOOR01");
    assert_eq!(top["align"]["scaleU"], 0.5, "{top}");
    // a light, and spawns: one CT spawn sunk into the floor on purpose (Checks should find it)
    ag.call("editor_add_entity", json!({"classname": "light", "origin": [0, 0, 128], "keys": {"_light": "255 255 230 300"}})).await;
    let stuck = ag.call("editor_add_entity", json!({"classname": "info_player_start", "origin": [-128, 0, 20]})).await;
    let stuck = ids(&stuck["created"]["entities"])[0];
    ag.call("editor_add_entity", json!({"classname": "info_player_deathmatch", "origin": [128, 0, 60]})).await;

    // ---- 2. the checks, and a fix by id ----------------------------------------------------
    let f = ag.call("editor_findings", json!({})).await;
    let stuck_finding = f["findings"].as_array().unwrap().iter().find(|x| x["kind"] == "stuck-in-solid" && ids(&x["entities"]).contains(&stuck)).unwrap_or_else(|| panic!("no stuck-in-solid for the sunk spawn: {f}")).clone();
    assert_eq!(stuck_finding["severity"], "error", "{stuck_finding}");
    assert!(stuck_finding["location"].is_array());
    let fix = stuck_finding["fixes"][0]["fixId"].as_str().unwrap().to_string();
    let fixed = ag.call("editor_apply_fix", json!({"fixId": fix})).await;
    assert_eq!(fixed["changed"], true, "{fixed}");
    let f2 = ag.call("editor_findings", json!({"kind": "stuck-in-solid"})).await;
    assert_eq!(f2["matched"], 0, "the spawn is out of the floor now: {f2}");
    // the move is one undo step, named in the history
    let hist = ag.call("editor_history", json!({})).await;
    assert!(hist.to_string().to_lowercase().contains("fix"), "{hist}");

    // ---- 3. the Budget ---------------------------------------------------------------------
    let b = ag.call("editor_budget", json!({})).await;
    let bars = b["bars"].as_array().unwrap();
    for id in ["precache_models", "precache_sounds"] {
        let bar = bars.iter().find(|x| x["id"] == id).unwrap_or_else(|| panic!("{id}: {b}"));
        assert!(bar["used"].as_u64().unwrap() >= 197, "the game's baseline is in: {bar}");
        assert_eq!(bar["status"], "green");
    }
    assert!(bars.iter().any(|x| x["group"] == "as you build" && x["id"].as_str().is_some_and(|i| i.contains("entit"))), "{b}");
    assert_eq!(b["warnings"], json!([]));

    // ---- 4. compile and read the result ----------------------------------------------------
    let started = ag.call("compile_start", json!({"preset": "fast", "name": "ls_agent_room"})).await;
    assert_eq!(started["ok"], true);
    let p = ag.call("compile_progress", json!({"from": 0})).await;
    assert_eq!(p["job"]["stage"], "done");
    assert_eq!(p["job"]["result"]["bsp"], "ls_agent_room.bsp");
    assert!(ag.calls.lock().unwrap().iter().any(|c| c.contains("\"preset\":\"fast\"")));

    // ---- 5. a Studio mode: an event, a node and wires --------------------------------------
    let d = ag.call("studio_draft_open", json!({"id": "agent_e2e", "from": "new", "name": "Agent test"})).await;
    assert_eq!(d["nodes"], json!([]));
    let r = ag.call("studio_edit", json!({"id": "agent_e2e", "ops": [
        {"op": "add_node", "type": "event.player_killed", "x": 0, "y": 0},
        {"op": "add_node", "type": "msg.chat", "x": 400, "y": 0},
    ]})).await;
    let (ev, chat) = (r["results"][0]["result"].as_str().unwrap().to_string(), r["results"][1]["result"].as_str().unwrap().to_string());
    let r = ag.call("studio_edit", json!({"id": "agent_e2e", "ops": [
        {"op": "connect", "from": {"node": ev, "pin": "then"}, "to": {"node": chat, "pin": "exec"}},
        {"op": "connect", "from": format!("{ev}.attacker"), "to": format!("{chat}.player")},
        {"op": "set_value", "node": chat, "param": "text", "value": "Nice shot!"},
    ]})).await;
    assert!(r["results"][0]["result"].as_str().is_some_and(|w| w.starts_with('w')), "{r}");
    let errors: Vec<&Value> = r["problems"].as_array().unwrap().iter().filter(|p| p["level"] == "error").collect();
    assert!(errors.is_empty(), "{errors:?}");
    // a wrong wire is refused and nothing changes
    let bad = catalog::call_tool(&ag.ctx, "studio_edit", json!({"id": "agent_e2e", "ops": [{"op": "connect", "from": format!("{ev}.headshot"), "to": format!("{chat}.exec")}]})).await;
    assert_eq!(bad["isError"], true, "{bad}");
    let view = ag.call("studio_draft", json!({"id": "agent_e2e"})).await;
    let chat_node = view["nodes"].as_array().unwrap().iter().find(|n| n["id"] == chat.as_str()).unwrap();
    assert_eq!(chat_node["settings"]["text"], "Nice shot!");
    assert!(chat_node["inputs"].as_array().unwrap().iter().any(|p| p["id"] == "exec" && p["wires"].is_array()), "{chat_node}");
    assert_eq!(view["errors"], 0);
    // undo and redo the wiring batch
    let u = ag.call("studio_draft_undo", json!({"id": "agent_e2e"})).await;
    assert_eq!(u["wires"], json!([]));
    let u = ag.call("studio_draft_redo", json!({"id": "agent_e2e"})).await;
    assert_eq!(u["wires"].as_array().unwrap().len(), 2);

    // ---- 6. compile it -----------------------------------------------------------------------
    let code = ag.call("studio_code", json!({"id": "agent_e2e", "draft": true})).await;
    assert!(code["text"].as_str().unwrap().contains("Nice shot!"), "the plugin says it");
    let built = ag.call("studio_compile", json!({"id": "agent_e2e", "draft": true})).await;
    assert_eq!(built["ok"], true, "{built}");
    assert_eq!(built["plugin"], gen::plugin_name("agent_e2e"));
    // installing into the game is asked about first; nothing else was
    let asked = ag.asked.lock().unwrap().clone();
    assert_eq!(asked.len(), 1, "{asked:?}");
    assert!(asked[0].contains("install"), "{asked:?}");
    assert!(ag.calls.lock().unwrap().iter().any(|c| c.starts_with("studio compile agent_e2e")));
}
