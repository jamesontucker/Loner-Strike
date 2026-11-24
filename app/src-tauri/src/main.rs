// Loner-Strike desktop app: a window showing the control panel, served by a
// small local web server (which phones on the same Wi-Fi can use too).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod admins;
mod app_api;
mod connect;
mod archive;
mod caps; // [Caps]
mod backup;
mod browser;
mod community;
mod consolehints;
mod control;
mod content;
mod crashes; // MCP: crash tracking for the player's servers
mod deeplink; // P5: loner-strike:// links (Open in Loner-Strike on the website)
mod localdata; // MCP: big folders in Local AppData on Windows, moved once from Roaming
mod roles; // MCP: community roles, the host's side (claim, /link relay, supporters, moderator invites)
mod hamguard; // MCP: the fixed Ham Sandwich kept in place before Play and local starts
mod procmatch; // MCP: a leftover process is ended only if it's still the one the app started
mod diagnostics;
mod compat; // W2: format versions across 1.x (docs/notes/Compat.md)
mod extras;
mod fastdl; // W2: fast downloads from Loner-Strike's mirror (dl.lonerstrike.com/fastdl/<server>)
mod game;
mod gamebin; // the game's binaries by system: .dll (Windows game, CrossOver too) or .so (native Linux game)
mod gamebanana;
mod hltv;
mod lists;
mod maps;
mod mcp;
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
mod goldsrc;
mod hlmodels;
pub mod editor;
mod tools;
mod navpack;
mod servercontent;
mod servers;
mod webstatus;
mod welcome;
mod setup;
mod setup_api;
mod sharing;
mod smartchat;
mod saved;
mod safety;
mod sounds;
mod steam;
mod sys;
mod voice;

use std::sync::{Arc, Mutex, OnceLock, RwLock};
use tauri::{WebviewUrl, WebviewWindowBuilder};

const PORT: u16 = 8027;

fn main() {
    // `loner-strike --mcp-stdio`: the AI agents bridge only (mcp/stdio.rs), no window
    if mcp::stdio::wanted() {
        return mcp::stdio::run();
    }

    // Windows: crashed servers exit at once instead of waiting behind a dialog (sys.rs)
    sys::quiet_crash_dialogs();
    // Windows: the big folders move from Roaming to Local AppData once (localdata.rs)
    localdata::start();

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
        // P5: loner-strike:// links (after single-instance, which hands a second launch's link here)
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_process::init())
        // the window opens where and how big it was last time (but not shown by the plugin: the
        // app shows it once the page is up, see show_when_loaded)
        .plugin(tauri_plugin_window_state::Builder::default().with_state_flags(tauri_plugin_window_state::StateFlags::all() & !tauri_plugin_window_state::StateFlags::VISIBLE).build())
        .plugin(tauri_plugin_notification::init())
        .on_menu_event(|app, event| menu_clicked(app, event.id().as_ref()))
        .setup(move |app| {
            let _ = state.app.set(app.handle().clone());
            deep_links(app);
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
            // P5 (QA4-F F5): servers made before 1.0 get the panel's RCON password file
            servers::migrate_panel_rcon();
            // voice for servers kept running while the app was closed
            servers::resume_voice();
            // MCP: community roles: the /link relay and supporters for claimed servers here
            roles::start();
            // servers' Loner-Strike updates, once nobody is playing
            setup_api::auto_update_loop(state.clone());
            // Linux servers' bans, admins and settings kept here too, once a day
            backup::auto_backup_loop(state.clone());
            // ready-made bot navigation for the maps this computer's game has (navpack.rs)
            navpack::start_loop();
            notify::watch(app.handle().clone(), state.clone());
            // crash alerts to Discord, for servers that asked for them
            crashes::watch();
            // where the fixed Ham Sandwich is, for putting it back before Play and local starts
            if let Some(res) = tauri::Manager::path(app).resolve("catalog", tauri::path::BaseDirectory::Resource).ok().and_then(|p| p.parent().map(std::path::Path::to_path_buf)) {
                hamguard::init(res);
            }
            // Smarter chat's llama-server: only while it's on and a game is hosted here
            smartchat::watch(state.clone());
            // AI agents (MCP), if turned on in Settings
            mcp::start(&state);
            #[cfg(target_os = "macos")]
            app.set_menu(mac_menu(app.handle())?)?;
            // LONER_UI_URL points the window at the React dev server while working on the UI
            let url: tauri::Url = std::env::var("LONER_UI_URL")
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
            // P5: the map editor's "Open in its own window" panels: window.open("") from the
            // app's page gets a real window sharing the page (so it can draw the panel into it);
            // anything else asked of window.open is refused here (outside links go through
            // /api/app/open-url to the browser).
            let handle = app.handle().clone();
            let next_panel = std::sync::atomic::AtomicU32::new(1);
            let start_url = url.clone();
            let window = WebviewWindowBuilder::new(app, "main", WebviewUrl::External(url))
                .on_new_window(move |url, features| {
                    if url.as_str() != "about:blank" {
                        return tauri::webview::NewWindowResponse::Deny;
                    }
                    let label = format!("panel-{}", next_panel.fetch_add(1, std::sync::atomic::Ordering::Relaxed));
                    let Ok(blank) = "about:blank".parse() else { return tauri::webview::NewWindowResponse::Deny };
                    // (P5: the page's own drag and drop, as in the main window)
                    match WebviewWindowBuilder::new(&handle, label, WebviewUrl::External(blank)).window_features(features).title("Loner-Strike").min_inner_size(260.0, 200.0).disable_drag_drop_handler().enable_clipboard_access().build() {
                        Ok(window) => tauri::webview::NewWindowResponse::Create { window },
                        Err(_) => tauri::webview::NewWindowResponse::Deny,
                    }
                })
                .title("Loner-Strike")
                // P5: Tauri's own file-drop handling swallows the page's HTML drag and drop (on
                // Windows always, on a Mac too): the editor drags prefabs, components, models,
                // sprites, sounds and textures onto the map, and nothing in the app takes files
                // dropped from the desktop
                .disable_drag_drop_handler()
                // (Windows: WebView2 asks the person before a page reads the clipboard; the editor
                // and the Studio paste .map text and nodes from other programs)
                .enable_clipboard_access()
                .inner_size(w, h)
                .min_inner_size(820.0, 600.0) // the sidebar scrolls when it doesn't fit
                .center()
                // 1.0.3: made hidden, in the app's own color, and shown once the page is up. Shown
                // at once, the window was resized (last size, then fit_on_screen) while WebView2
                // was still starting, and on Windows the page could stay a white square on black
                // until a restart (a tester, after updating)
                .visible(false)
                .background_color(tauri::window::Color(0x3e, 0x46, 0x37, 0xff))
                .on_page_load(|w, p| {
                    if p.event() == tauri::webview::PageLoadEvent::Finished {
                        PAGE_LOADED.store(true, std::sync::atomic::Ordering::SeqCst);
                        let _ = w.show();
                        let _ = w.set_focus();
                    }
                })
                .build()?;
            fit_on_screen(&window);
            show_when_loaded(window, start_url);
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

/// The main window's page has finished loading at least once.
static PAGE_LOADED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// The window shows when its page has loaded (on_page_load), else after 2 s anyway (a dark window
/// beats none: people start the app again); a page still not loaded after 8 s is reloaded, once
/// (a WebView2 start that hung, or security software holding it up: Microsoft's WebView2 notes).
fn show_when_loaded(window: tauri::WebviewWindow, url: tauri::Url) {
    use std::sync::atomic::Ordering;
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(2));
        if !PAGE_LOADED.load(Ordering::SeqCst) {
            let _ = window.show();
            let _ = window.set_focus();
        }
        std::thread::sleep(std::time::Duration::from_secs(6));
        if !PAGE_LOADED.load(Ordering::SeqCst) {
            crate::diagnostics::log_error("app start", "the page hadn't loaded 8 seconds after starting: loading it again");
            // (navigating, not reload(): with no page loaded yet there's nothing to reload; and
            // a new address, since the same one still loading is ignored)
            let mut again = url;
            again.query_pairs_mut().append_pair("retry", "1");
            let _ = window.navigate(again);
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

/// On Windows the updater ends the app itself to run the installer, so the usual clean-up on
/// quitting never runs: the same, first.
pub fn before_update_exit() {
    servers::stop_on_exit();
    if connect::DIRECT.lock().map(|d| d.on).unwrap_or(false) {
        connect::close_direct();
    }
    control::shutdown();
}

/// Looks for a newer release on GitHub and offers to install it.
/// Updates are signed; the updater refuses anything not signed with Loner-Strike's key.
async fn check_for_update(app: tauri::AppHandle) {
    use tauri_plugin_dialog::{DialogExt, MessageDialogButtons};
    use tauri_plugin_updater::UpdaterExt;

    let Ok(updater) = app.updater_builder().on_before_exit(before_update_exit).build() else { return };
    let Ok(Some(update)) = updater.check().await else { return };
    // the app shows it too (a line at the top and Settings), so "Later" isn't the last word on it
    *app_api::PENDING.lock().unwrap() = Some(update.clone());
    {
        use tauri::Manager;
        if let Some(w) = app.get_webview_window("main") {
            let _ = w.eval("window.dispatchEvent(new CustomEvent('ls-app-changed'))");
        }
    }
    // (the same rule as Settings' Install: never in the middle of a setup or a restore, which
    // the exit would cut off)
    let busy = || crate::setup::current().state == "running" || crate::backup::restoring_any();
    if busy() {
        return;
    }
    let install = app
        .dialog()
        .message(format!(
            "Loner-Strike {} is out (you have {}). Install it now? The app restarts when it's done.",
            update.version, update.current_version
        ))
        .title("Update available")
        .buttons(MessageDialogButtons::OkCancelCustom("Install".into(), "Later".into()))
        .blocking_show();
    if install && busy() {
        app.dialog().message("Something is being set up or restored. Update from Settings once it's finished.").title("Update later").blocking_show();
        return;
    }
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
    // (the sidebar's order: Game, then People)
    let sections = [
        ("play", "Play"), ("bots", "Bots"), ("match", "Rules"), ("fun", "Fun"), ("addons", "Add-ons"), ("extras", "Rotation & extras"),
        ("players", "Players"), ("community", "Moderation"), ("stats", "Stats"),
    ];
    let mut view = SubmenuBuilder::new(app, "Go");
    for (n, (id, label)) in sections.iter().enumerate() {
        view = view.item(&MenuItemBuilder::with_id(format!("section:{id}"), *label).accelerator(format!("CmdOrCtrl+{}", n + 1)).build(app)?);
    }
    let view = view
        .separator()
        .item(&MenuItemBuilder::with_id("section:admins", "Admins").build(app)?)
        .item(&MenuItemBuilder::with_id("section:welcome", "Welcome page").build(app)?)
        .separator()
        .item(&MenuItemBuilder::with_id("section:friends", "Play with friends").build(app)?)
        .item(&MenuItemBuilder::with_id("section:browser", "Find servers").build(app)?)
        .separator()
        .item(&MenuItemBuilder::with_id("section:maps", "Maps").build(app)?)
        .item(&MenuItemBuilder::with_id("section:mods", "Mods").build(app)?)
        .item(&MenuItemBuilder::with_id("section:skins", "Skins & models").build(app)?)
        .item(&MenuItemBuilder::with_id("section:sounds", "Sounds").build(app)?)
        .item(&MenuItemBuilder::with_id("section:more", "Get more").build(app)?)
        .separator()
        .item(&MenuItemBuilder::with_id("section:servers", "Servers").build(app)?)
        .item(&MenuItemBuilder::with_id("section:tools", "Tools").build(app)?)
        .build()?;
    let window = SubmenuBuilder::new(app, "Window").minimize().maximize().separator().fullscreen().separator().close_window().build()?;
    let help = SubmenuBuilder::new(app, "Help")
        .item(&MenuItemBuilder::with_id("link:site", "Loner-Strike website").build(app)?)
        .item(&MenuItemBuilder::with_id("link:discord", "Loner-Strike Discord").build(app)?)
        .item(&MenuItemBuilder::with_id("link:github", "Loner-Strike on GitHub").build(app)?)
        .build()?;
    MenuBuilder::new(app).items(&[&app_menu, &edit, &view, &window, &help]).build()
}

/// P5: loner-strike:// links: the scheme registered where the app needs to do it itself
/// (Windows, and Linux for the AppImage, which has no installed .desktop file; macOS reads it
/// from the bundle's Info.plist, the .deb from its own .desktop file), the link that started the app and
/// the ones that come while it runs. Each is checked (deeplink.rs), kept for the page, and the
/// page nudged to ask for it (it also asks when it loads).
fn deep_links(app: &tauri::App) {
    use tauri::Manager;
    use tauri_plugin_deep_link::DeepLinkExt;
    #[cfg(any(windows, target_os = "linux"))]
    if let Err(e) = app.deep_link().register_all() {
        eprintln!("couldn't register {}:// links: {e}", deeplink::SCHEME);
    }
    let nudge = |h: &tauri::AppHandle, links: Vec<String>| {
        if deeplink::open(&links) {
            if let Some(w) = h.get_webview_window("main") {
                let _ = w.unminimize();
                let _ = w.show();
                let _ = w.set_focus();
                let _ = w.eval("window.dispatchEvent(new CustomEvent('ls-open'))");
            }
        }
    };
    if let Ok(Some(urls)) = app.deep_link().get_current() {
        nudge(app.handle(), urls.iter().map(|u| u.to_string()).collect());
    }
    let h = app.handle().clone();
    app.deep_link().on_open_url(move |event| nudge(&h, event.urls().iter().map(|u| u.to_string()).collect()));
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
