//! A picture of a 3D view (map viewer, Replay), saved where the player picks with the
//! system's dialog: `POST /api/tools/screenshot?name=<file name>` with `{png: <base64>}`.
//! Only from the app's own pages (X-Panel); the body is checked to be a PNG and not huge.
//! Without the app's window (the browser dev server) it says so, and the page downloads the
//! picture itself.

use super::{blocking, json_answer, Fail};
use crate::server::Shared;
use axum::extract::{DefaultBodyLimit, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use base64::Engine;
use serde::Deserialize;
use serde_json::{json, Value};

/// A 4K view as a PNG is well under this.
const MAX_PNG: usize = 48 << 20;
const PNG_MAGIC: &[u8] = b"\x89PNG\r\n\x1a\n";
const NO_WINDOW: &str = "Saving pictures needs the Loner-Strike window.";

#[derive(Deserialize)]
struct Name {
    name: Option<String>,
}

#[derive(Deserialize)]
struct Body {
    png: String,
}

/// A file name to suggest: the page's, kept to plain characters, ending in .png.
fn file_name(asked: Option<&str>) -> String {
    let base: String = asked.unwrap_or("").chars().filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.')).take(80).collect();
    let base = base.trim_matches('.').trim_end_matches(".png").to_string();
    format!("{}.png", if base.is_empty() { "map-view" } else { &base })
}

/// The PNG in the body, checked.
fn decode(b: &Body) -> Result<Vec<u8>, Fail> {
    if b.png.len() > MAX_PNG / 3 * 4 + 4 {
        return Err(Fail::new(StatusCode::BAD_REQUEST, "That picture is too big."));
    }
    let bytes = base64::engine::general_purpose::STANDARD.decode(b.png.trim()).map_err(|_| Fail::new(StatusCode::BAD_REQUEST, "That isn't a picture."))?;
    if !bytes.starts_with(PNG_MAGIC) {
        return Err(Fail::new(StatusCode::BAD_REQUEST, "That isn't a picture."));
    }
    Ok(bytes)
}

async fn api_screenshot(State(st): State<Shared>, h: HeaderMap, Query(q): Query<Name>, Json(body): Json<Body>) -> Response {
    use tauri_plugin_dialog::DialogExt;
    if !crate::panel_api::from_panel(&h) {
        return (StatusCode::FORBIDDEN, Json(json!({"error": "Not allowed."}))).into_response();
    }
    let png = match decode(&body) {
        Ok(p) => p,
        Err(f) => return json_answer(Err(f)),
    };
    let Some(app) = st.app.get().cloned() else { return json_answer(Err(Fail::new(StatusCode::CONFLICT, NO_WINDOW))) };
    let name = file_name(q.name.as_deref());
    json_answer(
        blocking(move || -> Result<Value, Fail> {
            let Some(dest) = app.dialog().file().set_title("Save a picture of the view").set_file_name(&name).add_filter("PNG picture", &["png"]).blocking_save_file() else {
                return Ok(json!({"cancelled": true}));
            };
            let dest = dest.into_path().map_err(|e| Fail::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
            std::fs::write(&dest, png).map_err(|e| Fail::new(StatusCode::INTERNAL_SERVER_ERROR, format!("Couldn't save it: {e}")))?;
            Ok(json!({"ok": true, "message": format!("Saved {}.", dest.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default())}))
        })
        .await,
    )
}

pub fn routes() -> Router<Shared> {
    Router::new().route("/api/tools/screenshot", post(api_screenshot).layer(DefaultBodyLimit::max(MAX_PNG / 3 * 4 + 1024)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_and_pictures() {
        assert_eq!(file_name(Some("de_dust2-2026-11-12.png")), "de_dust2-2026-11-12.png");
        assert_eq!(file_name(Some("../../etc/passwd")), "etcpasswd.png");
        assert_eq!(file_name(Some("")), "map-view.png");
        assert_eq!(file_name(None), "map-view.png");
        let ok = base64::engine::general_purpose::STANDARD.encode([PNG_MAGIC, b"rest"].concat());
        assert!(decode(&Body { png: ok }).is_ok());
        assert!(decode(&Body { png: base64::engine::general_purpose::STANDARD.encode(b"GIF89a") }).is_err());
        assert!(decode(&Body { png: "not base64!".into() }).is_err());
    }

    #[tokio::test]
    async fn only_from_the_page_and_needs_the_window() {
        use tower::ServiceExt;
        use std::sync::{Arc, Mutex, OnceLock, RwLock};
        let st: Shared = Arc::new(crate::server::AppState {
            current: RwLock::new(None),
            hosts: RwLock::new(vec![]),
            lan_hosts: RwLock::new(vec![]),
            token: RwLock::new(String::new()),
            app: OnceLock::new(),
            phone: Mutex::new(crate::app_api::Phone::default()),
        });
        let app = routes().with_state(st);
        let body = json!({"png": base64::engine::general_purpose::STANDARD.encode([PNG_MAGIC, b"x"].concat())}).to_string();
        let req = |panel: bool| {
            let mut b = axum::http::Request::builder().method("POST").uri("/api/tools/screenshot?name=x").header("content-type", "application/json");
            if panel {
                b = b.header("x-panel", "1");
            }
            b.body(axum::body::Body::from(body.clone())).unwrap()
        };
        assert_eq!(app.clone().oneshot(req(false)).await.unwrap().status(), StatusCode::FORBIDDEN);
        let r = app.oneshot(req(true)).await.unwrap();
        let v: Value = serde_json::from_slice(&axum::body::to_bytes(r.into_body(), 1 << 16).await.unwrap()).unwrap();
        assert_eq!(v["error"], NO_WINDOW);
    }
}
