// Loner-Strike desktop app: a window showing the control panel, served by a
// small local web server (which phones on the same Wi-Fi can use too).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod game;
mod rcon;
mod server;
mod steam;

use std::sync::Arc;
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
    let hosts = vec![format!("127.0.0.1:{port}"), format!("localhost:{port}")];
    let state = Arc::new(server::AppState { game, hosts });

    tauri::Builder::default()
        .setup(move |app| {
            tauri::async_runtime::spawn(server::run(state.clone(), listener));
            // LONER_UI_URL points the window at the React dev server while building the new UI
            let url = std::env::var("LONER_UI_URL")
                .unwrap_or_else(|_| format!("http://127.0.0.1:{port}/"))
                .parse()
                .unwrap();
            WebviewWindowBuilder::new(app, "main", WebviewUrl::External(url))
                .title("Loner-Strike")
                .inner_size(1240.0, 880.0)
                .min_inner_size(380.0, 560.0)
                .build()?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("Loner-Strike couldn't start");
}
