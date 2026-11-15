//! What the person has selected in the map editor, for agents: "change this" means the
//! selection. The editor page reports it as it changes (tools/editor/selection.rs);
//! `editor_selection` reads it, `editor_select` sets it (the page selects it too, so the person
//! sees what the agent found or changed), and the typed edit tools take `selection: true` to
//! act on it.

use super::catalog::{err, get, obj, post, Done, Fail, Kind, Spec};
use super::Ctx;
use serde_json::{json, Map, Value};

pub fn specs() -> Vec<Spec> {
    use Kind::*;
    vec![
        Spec { name: "editor_selection", title: "Editor: what's selected", kind: Read, schema: super::catalog::none,
            description: "What the person has selected in the map editor right now (the page reports it as it changes): brush ids, entity ids and picked faces [[brush, face]], each in a few words (a brush's owner, size and textures; an entity's class, origin and name; a face's texture), the selection's bounds and center, and the tool and view in use. When the person says \"this\" or \"the selected ...\", it's this. Typed edit tools take selection: true to act on it." },
        Spec { name: "editor_select", title: "Editor: select", kind: Change,
            schema: || obj(json!({
                "brushes": {"type": "array", "items": {"type": "integer"}},
                "entities": {"type": "array", "items": {"type": "integer"}},
                "faces": {"type": "array", "items": {"type": "array", "items": {"type": "integer"}, "minItems": 2, "maxItems": 2}, "description": "[[brush, face], ...]"}}), &[]),
            description: "Selects things in the person's editor (brush, entity and face ids; nothing given clears it), to show them what was changed or found. Not an edit of the map: no undo step." },
    ]
}

/// The typed tools that take `selection: true`, and which of their fields it fills.
const SCOPED: &[(&str, &[&str])] = &[
    ("editor_transform", &["brushes", "entities"]),
    ("editor_delete", &["brushes", "entities"]),
    ("editor_duplicate", &["brushes", "entities"]),
    ("editor_array", &["brushes", "entities"]),
    ("editor_arrange", &["brushes", "entities"]),
    ("editor_group", &["brushes", "entities"]),
    ("editor_find", &["brushes", "entities"]),
    ("editor_replace", &["brushes", "entities"]),
    ("prefab_save", &["brushes", "entities"]),
    ("editor_hollow", &["brushes"]),
    ("editor_merge", &["brushes"]),
    ("editor_snap", &["brushes"]),
    ("editor_clip", &["brushes"]),
    ("editor_tie", &["brushes"]),
    ("editor_faces", &["brushes"]),
    ("editor_set_keys", &["entities"]),
    ("editor_set_key", &["entity"]),
    ("editor_set_texture", &["faces_or_brushes"]),
    ("editor_align_texture", &["faces"]),
    ("editor_wrap_texture", &["faces"]),
];

/// The schema a tool is listed with: `selection` added where it's taken.
pub fn with_selection(name: &str, mut schema: Value) -> Value {
    if SCOPED.iter().any(|(n, _)| *n == name) {
        schema["properties"]["selection"] = json!({"type": "boolean", "description": "Act on what the person has selected in the editor (editor_selection); fields given here win"});
    }
    schema
}

/// `selection: true` turned into the tool's own fields, from the editor's selection now.
pub async fn expand(ctx: &Ctx, name: &str, a: &Value) -> Result<Value, Fail> {
    if a.get("selection").is_none() {
        return Ok(a.clone());
    }
    let mut out: Map<String, Value> = a.as_object().cloned().unwrap_or_default();
    out.remove("selection");
    if a["selection"].as_bool() != Some(true) {
        return Ok(Value::Object(out));
    }
    let Some((_, fields)) = SCOPED.iter().find(|(n, _)| *n == name) else {
        return Err(err(format!("{name} doesn't act on the selection; give ids instead.")));
    };
    let sel = get(ctx, "/api/tools/editor/selection").await?;
    if sel["empty"] == true {
        return Err(err("Nothing is selected in the editor. Ask the person to select it, or give ids."));
    }
    fn given(out: &Map<String, Value>, k: &str) -> bool {
        out.get(k).is_some_and(|v| !v.is_null() && v.as_array().is_none_or(|x| !x.is_empty()))
    }
    for f in *fields {
        let has = |k: &str| given(&out, k);
        let (h, he, hf) = (has(f), has("entity"), has("faces"));
        match *f {
            "brushes" | "entities" if !h => {
                out.insert(f.to_string(), sel[*f].clone());
            }
            "entity" if !he => {
                let e = sel["entities"].as_array().cloned().unwrap_or_default();
                if e.len() != 1 {
                    return Err(err(format!("Select exactly one entity for {name} ({} are selected).", e.len())));
                }
                out.insert("entity".into(), e[0].clone());
            }
            "faces" if !hf => {
                if sel["faces"].as_array().is_none_or(Vec::is_empty) {
                    return Err(err("No faces are picked: the person picks faces with the Texture tool (or give faces)."));
                }
                out.insert("faces".into(), sel["faces"].clone());
            }
            // picked faces, else every face of the selected brushes
            "faces_or_brushes" if !hf => {
                let faces = match sel["faces"].as_array().filter(|f| !f.is_empty()) {
                    Some(f) => f.clone(),
                    None => sel["brushes"].as_array().into_iter().flatten().map(|b| json!([b, null])).collect(),
                };
                if faces.is_empty() {
                    return Err(err("Select brushes or faces first."));
                }
                out.insert("faces".into(), json!(faces));
            }
            _ => {}
        }
    }
    Ok(Value::Object(out))
}

pub async fn run(ctx: &Ctx, name: &str, a: &Value) -> Option<Done> {
    Some(match name {
        "editor_selection" => get(ctx, "/api/tools/editor/selection").await,
        "editor_select" => async {
            let ids = |k: &str| -> Result<Value, Fail> {
                match a.get(k) {
                    None | Some(Value::Null) => Ok(json!([])),
                    Some(Value::Array(v)) if v.len() <= 100_000 => Ok(Value::Array(v.clone())),
                    _ => Err(err(format!("{k} is a list."))),
                }
            };
            let v = post(ctx, "/api/tools/editor/selection", json!({"brushes": ids("brushes")?, "entities": ids("entities")?, "faces": ids("faces")?, "by": "agent"})).await?;
            Ok(v)
        }
        .await,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::super::*;
    use crate::server::AppState;
    use axum::body::Body;
    use axum::http::Request;
    use std::sync::RwLock;
    use tower::ServiceExt;

    /// The page selects three brushes; an agent reads the selection and paints it.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn an_agent_paints_what_the_person_selected() {
        let dir = std::env::temp_dir().join(format!("ls-mcp-sel-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let st: Shared = Arc::new(AppState {
            current: RwLock::new(None),
            hosts: RwLock::new(vec!["127.0.0.1:8027".into()]),
            lan_hosts: RwLock::new(vec![]),
            token: RwLock::new("testtoken0123456789".into()),
            app: OnceLock::new(),
            phone: Mutex::new(crate::app_api::Phone::default()),
        });
        let app = crate::tools::editor::routes_in(dir.clone()).with_state(st);
        let asked: Arc<Mutex<Vec<String>>> = Arc::default();
        let a2 = asked.clone();
        let ctx = Ctx { app: app.clone(), host: "127.0.0.1:8027".into(), changes: true, confirm: Arc::new(move |t| {
            a2.lock().unwrap().push(t);
            true
        }), log: Arc::new(Log::default()), st: None };
        let call = |name: &'static str, args: Value| {
            let ctx = &ctx;
            async move {
                let r = catalog::call_tool(ctx, name, args.clone()).await;
                let text = r["content"][0]["text"].as_str().unwrap_or("").to_string();
                assert_ne!(r["isError"], true, "{name} {args}: {text}");
                serde_json::from_str::<Value>(&text).unwrap_or(Value::String(text))
            }
        };
        call("editor_new", json!({})).await;
        let mut made = vec![];
        for i in 0..4 {
            let x = i as f64 * 128.0;
            let r = call("editor_add_box", json!({"min": [x, 0, 0], "max": [x + 64.0, 64, 64], "texture": "CRATE01"})).await;
            made.push(r["created"]["brushes"][0].as_u64().unwrap());
        }
        // the page reports three of them selected (as the editor page's report does)
        let body = json!({"brushes": &made[..3], "entities": [], "faces": [], "tool": "select", "view": "3d"});
        let req = Request::builder().method("POST").uri("/api/tools/editor/selection").header("host", "127.0.0.1:8027").header("x-panel", "1").header("content-type", "application/json").body(Body::from(body.to_string())).unwrap();
        assert!(app.clone().oneshot(req).await.unwrap().status().is_success());
        // the agent sees them, each in a few words, and where they are
        let sel = call("editor_selection", json!({})).await;
        assert_eq!(sel["brushes"], json!(&made[..3]), "{sel}");
        assert_eq!(sel["items"][0]["size"], json!([64.0, 64.0, 64.0]));
        assert_eq!(sel["items"][0]["textures"], json!(["CRATE01"]));
        assert_eq!(sel["bounds"]["max"], json!([320.0, 64.0, 64.0]));
        assert_eq!(sel["tool"], "select");
        // "make these metal": the selection, every face
        call("editor_set_texture", json!({"selection": true, "texture": "METAL01"})).await;
        let f = call("editor_faces", json!({"brushes": made})).await;
        for b in f["brushes"].as_array().unwrap() {
            let painted = b["faces"].as_array().unwrap().iter().all(|x| x["texture"] == "METAL01");
            assert_eq!(painted, b["brush"] != made[3], "only the selected ones: {b}");
        }
        // one undo step
        call("editor_undo", json!({})).await;
        let f = call("editor_faces", json!({"brushes": [made[0]]})).await;
        assert_eq!(f["brushes"][0]["faces"][0]["texture"], "CRATE01");
        // the agent shows the person the fourth one; the page follows by seq
        let before = sel["seq"].as_u64().unwrap();
        let s2 = call("editor_select", json!({"brushes": [made[3]]})).await;
        assert_eq!(s2["by"], "agent");
        assert!(s2["seq"].as_u64().unwrap() > before);
        assert_eq!(s2["tool"], "select", "the page's tool is kept");
        // the page's long look: a newer selection answers at once
        let seq = s2["seq"].as_u64().unwrap();
        let t0 = std::time::Instant::now();
        let req = Request::builder().uri(format!("/api/tools/editor/selection?after={}", seq - 1)).header("host", "127.0.0.1:8027").body(Body::empty()).unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        let v: Value = serde_json::from_slice(&axum::body::to_bytes(res.into_body(), 1 << 20).await.unwrap()).unwrap();
        assert_eq!(v["by"], "agent");
        assert!(t0.elapsed() < std::time::Duration::from_secs(2));
        // a tool that doesn't take the selection says so; nothing selected says so
        let r = catalog::call_tool(&ctx, "editor_add_box", json!({"selection": true, "min": [0, 0, 0], "max": [8, 8, 8]})).await;
        assert_eq!(r["isError"], true);
        call("editor_select", json!({})).await;
        let r = catalog::call_tool(&ctx, "editor_hollow", json!({"selection": true, "thickness": 8})).await;
        assert!(r["content"][0]["text"].as_str().unwrap().contains("Nothing is selected"), "{r}");
        // listed with the selection switch
        let tools = catalog::tool_list();
        let t = tools.iter().find(|t| t["name"] == "editor_set_texture").unwrap();
        assert!(t["inputSchema"]["properties"]["selection"].is_object());
        assert!(asked.lock().unwrap().is_empty(), "selection needs no question");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
