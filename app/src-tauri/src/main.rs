// Loner-Strike desktop app: a window showing the control panel, served by a
// small local web server (which phones on the same Wi-Fi can use too).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod admins;
mod app_api;
mod connect;
mod archive;
mod community;
mod content;
mod extras;
mod game;
mod gamebanana;
mod hltv;
mod lists;
mod maps;
mod modevote;
mod notify;
mod panel_api;
mod plugins;
mod prefs;
mod rcon;
mod remote;
mod scrim;
mod server;
mod navs;
mod servers;
mod webstatus;
mod welcome;
mod setup;
mod setup_api;
mod sharing;
mod smartchat;
mod sounds;
mod steam;
mod sys;

use std::sync::{Arc, Mutex, OnceLock, RwLock};
use tauri::{WebviewUrl, WebviewWindowBuilder};

const PORT: u16 = 8027;

fn main() {

    // Bind before opening the window so the page never loads too early.
    // If the Python panel already has 8027, take any free port.
    let listener = std::net::TcpListener::bind(("127.0.0.1", PORT))
        .or_else(|_| std::net::TcpListener::bind(("127.0.0.1", 0)))
        .expect("couldn't open a local port");
    let port = listener.local_addr().unwrap().port();
    let settings = app_api::load_settings();
    // the player's own game (servers are reached per request, see server::with_target)
    let game = servers::game_for("local", || app_api::pick_game(&settings));
    let state = Arc::new(server::AppState {
        current: RwLock::new(None),
        hosts: RwLock::new(vec![format!("127.0.0.1:{port}"), format!("localhost:{port}")]),
        lan_hosts: RwLock::new(Vec::new()),
        token: RwLock::new(settings.token.clone()),
        app: OnceLock::new(),
        phone: Mutex::new(app_api::Phone::default()),
    });
    state.set_game(game);
    let state_at_exit = state.clone();

    tauri::Builder::default()
        // one copy at a time (on Windows a second double-click would start another, with
        // its own background work and port): opening it again brings the window forward
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            use tauri::Manager;
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.unminimize();
                let _ = w.show();
                let _ = w.set_focus();
            }
        }))
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_process::init())
        // the window opens where and how big it was last time
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .plugin(tauri_plugin_notification::init())
        .on_menu_event(|app, event| menu_clicked(app, event.id().as_ref()))
        .setup(move |app| {
            let _ = state.app.set(app.handle().clone());
            tauri::async_runtime::spawn(server::serve(state.clone(), listener));
            if settings.phone {
                app_api::start_phone(&state);
            }
            if !settings.manual_updates {
                tauri::async_runtime::spawn(check_for_update(app.handle().clone()));
            }
            connect::renew_loop();
            // dedicated servers' daily restarts (Server extras, Schedule)
            servers::schedule_restarts();
            servers::sync_loop();
            notify::watch(app.handle().clone(), state.clone());
            // Smarter chat's llama-server: only while it's on and a game is hosted here
            smartchat::watch(state.clone());
            #[cfg(target_os = "macos")]
            app.set_menu(mac_menu(app.handle())?)?;
            // LONER_UI_URL points the window at the React dev server while working on the UI
            let url = std::env::var("LONER_UI_URL")
                .unwrap_or_else(|_| format!("http://127.0.0.1:{port}/"))
                .parse()
                .unwrap();
            // a little roomier than before, but never taller or wider than the screen
            // (a 13" laptop is about 900 points tall, menu bar and Dock included)
            let (mut w, mut h) = (1320.0, 980.0);
            if let Ok(Some(m)) = app.primary_monitor() {
                let size = m.size().to_logical::<f64>(m.scale_factor());
                w = f64::min(w, size.width - 40.0);
                h = f64::min(h, size.height - 60.0);
            }
            WebviewWindowBuilder::new(app, "main", WebviewUrl::External(url))
                .title("Loner-Strike")
                .inner_size(w, h)
                .min_inner_size(820.0, 600.0) // the sidebar scrolls when it doesn't fit
                .center()
                .build()?;
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("Loner-Strike couldn't start")
        .run(move |_, event| {
            // stop dedicated servers and close the router port when the app quits
            if let tauri::RunEvent::Exit = event {
                smartchat::shutdown(&state_at_exit);
                // unless the player wants them left running
                if !app_api::load_settings().keep_servers {
                    servers::stop_all();
                }
                if connect::DIRECT.lock().map(|d| d.on).unwrap_or(false) {
                    connect::close_direct();
                }
            }
        });
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

/// The macOS menu bar: the usual app, Edit and Window menus, plus the panel's sections.
#[cfg(target_os = "macos")]
fn mac_menu(app: &tauri::AppHandle) -> tauri::Result<tauri::menu::Menu<tauri::Wry>> {
    use tauri::menu::{AboutMetadata, MenuBuilder, MenuItemBuilder, PredefinedMenuItem, SubmenuBuilder};
    let about = AboutMetadata {
        name: Some("Loner-Strike".into()),
        version: Some(app.package_info().version.to_string()),
        comments: Some("Play Counter-Strike like it's 2004, alone or with the friends you don't have.".into()),
        website: Some("https://github.com/jamesontucker/Loner-Strike".into()),
        ..Default::default()
    };
    let app_menu = SubmenuBuilder::new(app, "Loner-Strike")
        .item(&PredefinedMenuItem::about(app, Some("About Loner-Strike"), Some(about))?)
        .item(&MenuItemBuilder::with_id("updates", "Check for Updates…").build(app)?)
        .separator()
        .item(&MenuItemBuilder::with_id("section:settings", "Settings…").accelerator("CmdOrCtrl+,").build(app)?)
        .separator()
        .services()
        .separator()
        .hide()
        .hide_others()
        .show_all()
        .separator()
        .quit()
        .build()?;
    let edit = SubmenuBuilder::new(app, "Edit").undo().redo().separator().cut().copy().paste().select_all().build()?;
    // ⌘1-⌘9 go to the first nine sections, the same as the keys in the window
    let sections = [
        ("play", "Play"), ("maps", "Maps"), ("bots", "Bots"), ("match", "Match"), ("fun", "Fun"), ("players", "Players"),
        ("addons", "Add-ons"), ("friends", "Play with friends"), ("admins", "Admins"),
    ];
    let mut view = SubmenuBuilder::new(app, "Go");
    for (n, (id, label)) in sections.iter().enumerate() {
        view = view.item(&MenuItemBuilder::with_id(format!("section:{id}"), *label).accelerator(format!("CmdOrCtrl+{}", n + 1)).build(app)?);
    }
    let view = view
        .separator()
        .item(&MenuItemBuilder::with_id("section:welcome", "Welcome page").build(app)?)
        .item(&MenuItemBuilder::with_id("section:more", "Get more").build(app)?)
        .item(&MenuItemBuilder::with_id("section:mods", "Mods").build(app)?)
        .item(&MenuItemBuilder::with_id("section:servers", "Servers").build(app)?)
        .build()?;
    let window = SubmenuBuilder::new(app, "Window").minimize().maximize().separator().fullscreen().separator().close_window().build()?;
    let help = SubmenuBuilder::new(app, "Help").item(&MenuItemBuilder::with_id("github", "Loner-Strike on GitHub").build(app)?).build()?;
    MenuBuilder::new(app).items(&[&app_menu, &edit, &view, &window, &help]).build()
}

/// A menu item for the page: it listens for "ls-menu" (section:<id>, updates).
fn menu_clicked(app: &tauri::AppHandle, id: &str) {
    use tauri::Manager;
    if id == "github" {
        #[cfg(target_os = "macos")]
        let _ = std::process::Command::new("open").arg("https://github.com/jamesontucker/Loner-Strike").spawn().map(crate::sys::reap);
        return;
    }
    if !(id == "updates" || id.starts_with("section:")) || !id.chars().all(|c| c.is_ascii_alphanumeric() || c == ':') {
        return;
    }
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.eval(&format!("window.dispatchEvent(new CustomEvent('ls-menu', {{ detail: '{id}' }}))"));
    }
}
