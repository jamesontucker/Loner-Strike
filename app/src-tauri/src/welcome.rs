//! The welcome page players see when they join (the game's MOTD window), per game
//! or server. gm_pubserver reads these from the Game Mode Menu's data folder:
//!   welcome_url.txt   a web page the game's browser opens instead
//!   welcome.html      the host's own page, {server} {player} {map} {mode} {players} filled in
//!   welcome_note.txt  a few lines under the built-in page
//! Nothing there: the built-in page.

use crate::panel_api::from_panel;
use crate::server::Shared;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::Path;

/// The game's MOTD window takes about 1.5 KB; placeholders grow when filled in.
pub const MAX_HTML: usize = 1300;
const MAX_NOTE: usize = 300;

fn read(dir: &Path, name: &str) -> String {
    std::fs::read_to_string(dir.join(name)).unwrap_or_default()
}

fn current(data: &Path) -> Value {
    let url = read(data, "welcome_url.txt").trim().to_string();
    let html = read(data, "welcome.html");
    let style = if !url.is_empty() {
        "url"
    } else if !html.trim().is_empty() {
        "custom"
    } else {
        "builtin"
    };
    // keep what isn't in use, so switching back and forth loses nothing
    let draft: Value = serde_json::from_str(&read(data, "welcome_draft.json")).unwrap_or(json!({}));
    json!({
        "style": style,
        "note": read(data, "welcome_note.txt").trim(),
        "html": if html.trim().is_empty() { draft["html"].clone() } else { json!(html) },
        "url": if url.is_empty() { draft["url"].clone() } else { json!(url) },
        "max": MAX_HTML,
    })
}

#[derive(Deserialize)]
struct Save {
    style: String,
    #[serde(default)]
    note: String,
    #[serde(default)]
    html: String,
    #[serde(default)]
    url: String,
}

fn save(data: &Path, req: &Save) -> Result<(), String> {
    let note = req.note.trim();
    if note.chars().count() > MAX_NOTE {
        return Err(format!("Keep the message under {MAX_NOTE} characters."));
    }
    let url = req.url.trim();
    if req.style == "url" && !(url.starts_with("http://") || url.starts_with("https://")) {
        return Err("Enter a web address starting with https://".into());
    }
    if url.len() > 200 || url.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err("That web address doesn't look right.".into());
    }
    if req.html.len() > MAX_HTML {
        return Err(format!("The page is {} characters; the game only shows about {MAX_HTML}.", req.html.len()));
    }
    if req.style == "custom" && req.html.trim().is_empty() {
        return Err("Write the page first, or pick the built-in one.".into());
    }
    let w = |name: &str, text: &str| -> Result<(), String> {
        let p = data.join(name);
        if text.is_empty() {
            match std::fs::remove_file(&p) {
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.to_string()),
                _ => Ok(()),
            }
        } else {
            // the plugin reads Latin-1, like everything the game shows
            crate::sys::write_latin1(&p, text).map_err(|e| e.to_string())
        }
    };
    std::fs::create_dir_all(data).map_err(|e| e.to_string())?;
    w("welcome_note.txt", note)?;
    w("welcome_url.txt", if req.style == "url" { url } else { "" })?;
    w("welcome.html", if req.style == "custom" { &req.html } else { "" })?;
    w("welcome_draft.json", &json!({"html": req.html, "url": url}).to_string())
}

async fn api_get(State(st): State<Shared>) -> Json<Value> {
    Json(match st.game() {
        Some(g) => current(&g.data),
        None => json!({"error": "No game found."}),
    })
}

async fn api_set(State(st): State<Shared>, headers: HeaderMap, Json(req): Json<Save>) -> (StatusCode, Json<Value>) {
    if !from_panel(&headers) {
        return (StatusCode::FORBIDDEN, Json(json!({"error": "forbidden"})));
    }
    if !["builtin", "custom", "url"].contains(&req.style.as_str()) {
        return (StatusCode::BAD_REQUEST, Json(json!({"error": "Unknown style."})));
    }
    let Some(g) = st.game() else {
        return (StatusCode::OK, Json(json!({"error": "No game found."})));
    };
    match save(&g.data, &req) {
        Ok(()) => (StatusCode::OK, Json(json!({"ok": true}))),
        Err(e) => (StatusCode::OK, Json(json!({"error": e}))),
    }
}

pub fn routes() -> Router<Shared> {
    Router::new().route("/api/welcome", get(api_get).post(api_set))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn styles_round_trip() {
        let dir = std::env::temp_dir().join(format!("ls-welcome-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let s = |style: &str, html: &str, url: &str| Save { style: style.into(), note: "No camping".into(), html: html.into(), url: url.into() };
        save(&dir, &s("custom", "<h1>{server}</h1>", "")).unwrap();
        assert_eq!(current(&dir)["style"], "custom");
        save(&dir, &s("url", "<h1>{server}</h1>", "https://example.com/motd")).unwrap();
        let v = current(&dir);
        assert_eq!(v["style"], "url");
        assert_eq!(v["html"], "<h1>{server}</h1>", "the unused page is kept as a draft");
        save(&dir, &s("builtin", "", "")).unwrap();
        assert_eq!(current(&dir)["style"], "builtin");
        assert_eq!(current(&dir)["note"], "No camping");
        assert!(save(&dir, &s("url", "", "javascript:alert(1)")).is_err());
        assert!(save(&dir, &s("custom", &"x".repeat(MAX_HTML + 1), "")).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
