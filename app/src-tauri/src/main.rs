// Loner-Strike desktop app: a window showing the control panel, served by a
// small local web server (which phones on the same Wi-Fi can use too).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod admins;
mod app_api;
mod connect;
mod archive;
mod backup;
mod browser;
mod community;
mod consolehints;
mod control;
mod content;
mod diagnostics;
mod extras;
mod game;
mod gamebanana;
mod hltv;
mod lists;
mod maps;
mod mapcopy;
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
mod servercontent;
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
            // friends helping run a shared server reach this app here (control/)
            control::listen(&state);
            // and the ones running on Linux servers kept at this app's version
            control::refresh_agents(&state);
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
            // servers' Loner-Strike updates, once nobody is playing
            setup_api::auto_update_loop(state.clone());
            // Linux servers' bans, admins and settings kept here too, once a day
            backup::auto_backup_loop(state.clone());
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
            let window = WebviewWindowBuilder::new(app, "main", WebviewUrl::External(url))
                .title("Loner-Strike")
                .inner_size(w, h)
                .min_inner_size(820.0, 600.0) // the sidebar scrolls when it doesn't fit
                .center()
                .build()?;
            fit_on_screen(&window);
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("Loner-Strike couldn't start")
        .run(move |_, event| {
            // stop dedicated servers and close the router port when the app quits
            if let tauri::RunEvent::Exit = event {
                smartchat::shutdown(&state_at_exit);
                // except the ones set to keep running (each server's own setting)
                servers::stop_on_exit();
                if connect::DIRECT.lock().map(|d| d.on).unwrap_or(false) {
                    connect::close_direct();
                }
                // shared control's router forwarding too
                control::shutdown();
            }
        });
}

/// The window comes back where and how big it was last time, which may be on a bigger
/// monitor than this one (a laptop away from its desk): too big for the screen it's on,
/// or mostly off it, it's made to fit and centred.
fn fit_on_screen(window: &tauri::WebviewWindow) {
    let Ok(Some(m)) = window.current_monitor().or_else(|_| window.primary_monitor()) else { return };
    let (Ok(pos), Ok(size)) = (window.outer_position(), window.outer_size()) else { return };
    let (area, scale) = (m.work_area(), m.scale_factor());
    let (ax, ay, aw, ah) = (area.position.x, area.position.y, area.size.width as i32, area.size.height as i32);
    let (w, h) = (size.width as i32, size.height as i32);
    let too_big = w > aw || h > ah;
    // less than half of it on this screen
    let seen_w = (pos.x + w).min(ax + aw) - pos.x.max(ax);
    let seen_h = (pos.y + h).min(ay + ah) - pos.y.max(ay);
    let off = seen_w < w / 2 || seen_h < h / 2 || pos.y < ay;
    if !too_big && !off {
        return;
    }
    if too_big {
        // a little room around it, never under the minimum
        let fit = tauri::PhysicalSize::new(w.min(aw - (40.0 * scale) as i32).max(1) as u32, h.min(ah - (40.0 * scale) as i32).max(1) as u32);
        let _ = window.set_size(tauri::Size::Physical(fit));
    }
    let _ = window.center();
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
        website: Some("https://lonerstrike.com".into()),
        website_label: Some("lonerstrike.com".into()),
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
        .item(&MenuItemBuilder::with_id("section:browser", "Find servers").build(app)?)
        .item(&MenuItemBuilder::with_id("section:servers", "Servers").build(app)?)
        .build()?;
    let window = SubmenuBuilder::new(app, "Window").minimize().maximize().separator().fullscreen().separator().close_window().build()?;
    let help = SubmenuBuilder::new(app, "Help")
        .item(&MenuItemBuilder::with_id("link:site", "Loner-Strike website").build(app)?)
        .item(&MenuItemBuilder::with_id("link:discord", "Loner-Strike Discord").build(app)?)
        .item(&MenuItemBuilder::with_id("link:github", "Loner-Strike on GitHub").build(app)?)
        .build()?;
    MenuBuilder::new(app).items(&[&app_menu, &edit, &view, &window, &help]).build()
}

/// A menu item for the page: it listens for "ls-menu" (section:<id>, updates).
fn menu_clicked(app: &tauri::AppHandle, id: &str) {
    use tauri::Manager;
    // the Help menu's links, opened in the browser
    let link = match id {
        "link:site" => Some("https://lonerstrike.com"),
        "link:discord" => Some("https://discord.gg/K9eBHVVwZE"),
        "link:github" => Some("https://github.com/jamesontucker/Loner-Strike"),
        _ => None,
    };
    if let Some(url) = link {
        #[cfg(target_os = "macos")]
        let _ = std::process::Command::new("open").arg(url).spawn().map(crate::sys::reap);
        let _ = url;
        return;
    }
    if !(id == "updates" || id.starts_with("section:")) || !id.chars().all(|c| c.is_ascii_alphanumeric() || c == ':') {
        return;
    }
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.eval(&format!("window.dispatchEvent(new CustomEvent('ls-menu', {{ detail: '{id}' }}))"));
    }
}
