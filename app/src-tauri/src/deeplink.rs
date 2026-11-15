//! `loner-strike://` links (P5): the website's "Open in Loner-Strike" opens a Community creation's
//! page in the app. Only one form is understood, checked strictly, and it only ever opens the
//! creation's details page (Tools > Community): installing stays behind Try it and its usual
//! confirm.
//!
//!   loner-strike://community/<mod|mods|spray|sprays>/<GameBanana id>
//!
//! Anything else (another host, more or fewer parts, a query, letters in the id, an id of 0 or
//! over 12 digits) is ignored. A link that arrives while the app runs goes straight to the page
//! (the same window event the macOS menu uses); one that started the app waits here until the
//! page asks for it once it's loaded (POST /api/app/deep-link from the app's own page, which hands it over once).

use crate::server::Shared;
use axum::http::HeaderMap;
use axum::response::IntoResponse;
use axum::routing::post;
use axum::{Json, Router};
use serde_json::json;
use std::sync::Mutex;

pub const SCHEME: &str = "loner-strike";

/// A link that came before the page was ready (only the latest is kept).
static PENDING: Mutex<Option<String>> = Mutex::new(None);

/// The app address a link opens ("community:mod:123"), or None for anything not understood.
pub fn address_of(link: &str) -> Option<String> {
    // (control characters anywhere, even at the ends, aren't a link; surrounding spaces are fine)
    if link.len() > 200 || !link.is_ascii() || link.chars().any(|c| c.is_ascii_control()) {
        return None;
    }
    let link = link.trim_matches(' ');
    if link.contains(' ') {
        return None;
    }
    let rest = link.strip_prefix(&format!("{SCHEME}://")).or_else(|| link.strip_prefix(&format!("{}://", SCHEME.to_ascii_uppercase())))?;
    // (a trailing slash is fine; a query, a fragment, user info, a port or extra parts aren't)
    if rest.contains(['?', '#', '@', '%', '\\', ':']) {
        return None;
    }
    let parts: Vec<&str> = rest.trim_end_matches('/').split('/').collect();
    let [host, model, id] = parts.as_slice() else { return None };
    if !host.eq_ignore_ascii_case("community") {
        return None;
    }
    let model = match model.to_ascii_lowercase().as_str() {
        "mod" | "mods" => "mod",
        "spray" | "sprays" => "spray",
        _ => return None,
    };
    if id.is_empty() || id.len() > 12 || !id.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let n: u64 = id.parse().ok()?;
    if n == 0 {
        return None;
    }
    Some(format!("community:{model}:{n}"))
}

/// Links the system handed the app: the first one understood waits for the page (true if one
/// did). The caller then nudges the page, which asks for it (also done when the page loads, so a
/// link that came before the page was ready isn't lost).
pub fn open(links: &[String]) -> bool {
    let Some(address) = links.iter().find_map(|l| address_of(l)) else {
        if !links.is_empty() {
            eprintln!("deep link ignored: not one Loner-Strike opens");
        }
        return false;
    };
    *PENDING.lock().unwrap_or_else(|e| e.into_inner()) = Some(address);
    true
}

/// The waiting link, once (the page asks when it has loaded).
pub fn take() -> Option<String> {
    PENDING.lock().unwrap_or_else(|e| e.into_inner()).take()
}

async fn api_take(h: HeaderMap) -> impl IntoResponse {
    if !crate::panel_api::from_panel(&h) {
        return (axum::http::StatusCode::FORBIDDEN, Json(json!({"error": "forbidden"})));
    }
    (axum::http::StatusCode::OK, Json(json!({"open": take()})))
}

pub fn routes() -> Router<Shared> {
    Router::new().route("/api/app/deep-link", post(api_take))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_community_creation_link_is_understood() {
        for (link, want) in [
            ("loner-strike://community/mod/512345", Some("community:mod:512345")),
            ("loner-strike://community/mods/512345/", Some("community:mod:512345")),
            ("loner-strike://community/spray/77", Some("community:spray:77")),
            ("LONER-STRIKE://Community/MODS/0042", Some("community:mod:42")),
            (" loner-strike://community/sprays/9 ", Some("community:spray:9")),
        ] {
            assert_eq!(address_of(link).as_deref(), want, "{link}");
        }
        for bad in [
            "loner-strike://community/mod/0",
            "loner-strike://community/mod/1234567890123",
            "loner-strike://community/mod/12a",
            "loner-strike://community/mod/-5",
            "loner-strike://community/mod/5?install=1",
            "loner-strike://community/mod/5#x",
            "loner-strike://community/mod/5/extra",
            "loner-strike://community/mod",
            "loner-strike://community/wip/5",
            "loner-strike://community/../settings/5",
            "loner-strike://settings/mod/5",
            "loner-strike://user@community/mod/5",
            "loner-strike://community:80/mod/5",
            "loner-strike://community/mod/%35",
            "loner-strike://community/mod/5\n",
            "loner-strike://community/mod/５",
            "https://community/mod/5",
            "loner-strike:community/mod/5",
            "",
        ] {
            assert_eq!(address_of(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn a_link_waits_until_the_page_asks() {
        // (the one PENDING slot: these steps in one test)
        assert!(!open(&["https://evil.example/x".into()]));
        assert_eq!(take(), None);
        assert!(open(&["nope".into(), "loner-strike://community/spray/7".into()]));
        assert_eq!(take().as_deref(), Some("community:spray:7"));
        assert_eq!(take(), None, "handed over once");
    }
}
