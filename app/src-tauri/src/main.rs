// Loner-Strike desktop app: a window showing the control panel, served by a
// small local web server (which phones on the same Wi-Fi can use too).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app_api;
mod archive;
mod content;
mod game;
mod gamebanana;
mod maps;
mod panel_api;
mod rcon;
mod server;
mod setup;
mod setup_api;
mod sharing;
mod steam;
mod sys;

use std::sync::{Arc, Mutex, OnceLock, RwLock};
use tauri::{WebviewUrl, WebviewWindowBuilder};

const PORT: u16 = 8027;

fn main() {
    // First install with a game in it; the setup wizard will let you choose
    let game = steam::find_installs()
        .into_iter()
        .find_map(|i| i.games.first().map(|g| game::Game::new(&i.hl, g)));

    // Bind before opening the window so the page never loads too early.
    // If the Python panel already has 8027, take any free port.
    let listener = std::net::TcpListener::bind(("127.0.0.1", PORT))
        .or_else(|_| std::net::TcpListener::bind(("127.0.0.1", 0)))
        .expect("couldn't open a local port");
    let port = listener.local_addr().unwrap().port();
    let settings = app_api::load_settings();
    let state = Arc::new(server::AppState {
        game,
        hosts: RwLock::new(vec![format!("127.0.0.1:{port}"), format!("localhost:{port}")]),
        lan_hosts: RwLock::new(Vec::new()),
        token: settings.token.clone(),
        app: OnceLock::new(),
        phone: Mutex::new(app_api::Phone::default()),
    });

    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_process::init())
        .setup(move |app| {
            let _ = state.app.set(app.handle().clone());
            tauri::async_runtime::spawn(server::serve(state.clone(), listener));
            if settings.phone {
                app_api::start_phone(&state);
            }
            tauri::async_runtime::spawn(check_for_update(app.handle().clone()));
            // LONER_UI_URL points the window at the React dev server while working on the UI
            let url = std::env::var("LONER_UI_URL")
                .unwrap_or_else(|_| format!("http://127.0.0.1:{port}/"))
                .parse()
                .unwrap();
            WebviewWindowBuilder::new(app, "main", WebviewUrl::External(url))
                .title("Loner-Strike")
                .inner_size(1240.0, 860.0)
                .min_inner_size(820.0, 560.0)
                .build()?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("Loner-Strike couldn't start");
}

/// Looks for a newer release on GitHub and offers to install it.
/// Updates are signed; the updater refuses anything not signed with Loner-Strike's key.
async fn check_for_update(app: tauri::AppHandle) {
    use tauri_plugin_dialog::{DialogExt, MessageDialogButtons};
    use tauri_plugin_updater::UpdaterExt;

    let Ok(updater) = app.updater() else { return };
    let Ok(Some(update)) = updater.check().await else { return };
    let install = app
        .dialog()
        .message(format!(
            "Loner-Strike {} is out (you have {}). Install it now? The app restarts when it's done.",
            update.version, update.current_version
        ))
        .title("Update available")
        .buttons(MessageDialogButtons::OkCancelCustom("Install".into(), "Later".into()))
        .blocking_show();
    if install && update.download_and_install(|_, _| {}, || {}).await.is_ok() {
        app.restart();
    }
}
