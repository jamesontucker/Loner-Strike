//! Development only (not built into the app): every Tools route over a real game folder, for
//! running the Tools pages in a browser without the app. `npm run tools:dev` in app/ui starts
//! this and the Vite dev server together (docs/TOOLS-DEV.md); by hand:
//! `LS_TEST_GAME=<Half-Life folder> cargo test serve_tools_dev -- --ignored --nocapture`.
//!
//! - LS_TEST_GAME: the Half-Life folder (cstrike/ and czero/ in it). Edits that save (nav
//!   fixer, callouts, map doctor) write there: point it at a copy to keep the real one as is.
//! - LS_TEST_MOD: cstrike (default) or czero.
//! - LS_DEV_PORT: the port (default 8027, what Vite proxies to; the script picks 8037 so it
//!   doesn't clash with a running app).
//!
//! The app's own data (health check reports, sessions, downloads) goes to a temp folder.
//! The few non-tools routes the pages ask for answer as an app with nothing going on would.

#[cfg(test)]
mod tests {
    use crate::server::{AppState, Shared};
    use axum::routing::get;
    use axum::{Json, Router};
    use serde_json::json;
    use std::sync::{Arc, Mutex, OnceLock, RwLock};

    #[tokio::test(flavor = "multi_thread")]
    #[ignore]
    async fn serve_tools_dev() {
        let Ok(hl) = std::env::var("LS_TEST_GAME") else {
            eprintln!("Set LS_TEST_GAME to a Half-Life folder.");
            return;
        };
        let game = std::env::var("LS_TEST_MOD").unwrap_or_else(|_| "cstrike".into());
        let port: u16 = std::env::var("LS_DEV_PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(8027);
        let app_dir = std::env::temp_dir().join("ls-tools-dev");
        let _ = std::fs::create_dir_all(&app_dir);
        // everything the tools keep (editor recent/autosave/backups, prefabs, Studio graphs) in
        // the temp folder too, as TOOLS-DEV.md says, unless LS_APP_DIR picks one (P5)
        if std::env::var_os("LS_APP_DIR").is_none() {
            std::env::set_var("LS_APP_DIR", &app_dir);
        }
        // refuses to start if any app path (settings.json, the cache, downloads...) would be
        // outside it (QA4-CORE F6)
        let app_dir = crate::sys::app_dir();
        let g = crate::game::Game::with_app_dir(std::path::Path::new(&hl), &game, &app_dir);
        // backups (safety.rs) of what the tools save, in the temp app folder
        crate::safety::use_game(&g);
        let host = format!("127.0.0.1:{port}");
        let st: Shared = Arc::new(AppState {
            current: RwLock::new(Some(Arc::new(g))),
            hosts: RwLock::new(vec![host.clone(), format!("localhost:{port}")]),
            lan_hosts: RwLock::new(vec![]),
            token: RwLock::new("devtoken0123456789".into()),
            app: OnceLock::new(),
            phone: Mutex::new(crate::app_api::Phone::default()),
        });
        let extra: Router<Shared> = Router::new()
            // map doctor: "learn this map" (needs the app's hidden server, so nothing runs here)
            .route("/api/navs/learn", get(|| async { Json(json!({"result": null, "running": false})) }).post(|| async { Json(json!({"error": "Learning a map needs the app."})) }))
            // textures & models: GameBanana downloads in progress
            .route("/api/gb/jobs", get(|| async { Json(json!({"jobs": []})) }))
            .route("/api/tools/dev", get(|| async { Json(json!({"dev": true})) }));
        // History & restore, and "Undo" after a save (safety.rs), like the app's listener
        let app = super::super::local_routes().merge(extra).merge(crate::safety::routes()).layer(axum::middleware::from_fn(crate::safety::mark_saves)).with_state(st);
        let l = tokio::net::TcpListener::bind(&host).await.unwrap_or_else(|e| panic!("can't listen on {host}: {e} (is the app or another dev server running? set LS_DEV_PORT)"));
        println!("tools routes on http://{host} for {hl} ({game})");
        axum::serve(l, app).await.unwrap();
    }
}
