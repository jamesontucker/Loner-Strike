//! A desktop notification when someone joins your game or one of your servers,
//! for when Loner-Strike is in the background (Settings: "Tell me when someone joins"),
//! and one for each player report (say /report, see community.rs).

use crate::server::Shared;
use std::collections::{HashMap, HashSet};
use std::time::Duration;
use tauri::Manager;
use tauri_plugin_notification::NotificationExt;

/// Real people in a game right now (not bots, not you): (userid, name).
fn people(g: &crate::game::Game) -> Option<HashSet<(i64, String)>> {
    let v = g.state();
    if v["online"] != true {
        return None;
    }
    Some(
        v["players"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|p| p["bot"].as_i64() != Some(1) && p["host"].as_i64() != Some(1))
            .map(|p| (p["uid"].as_i64().unwrap_or(0), p["name"].as_str().unwrap_or("Someone").to_string()))
            .collect(),
    )
}

pub fn watch(app: tauri::AppHandle, st: Shared) {
    std::thread::spawn(move || {
        // what each game looked like last time; a game seen for the first time doesn't notify
        let mut seen: HashMap<String, HashSet<(i64, String)>> = HashMap::new();
        loop {
            std::thread::sleep(Duration::from_secs(5));
            let mut games: Vec<(String, String, std::sync::Arc<crate::game::Game>)> = Vec::new();
            if let Some(g) = st.local_game() {
                games.push(("local".into(), "your game".into(), g));
            }
            for e in crate::servers::load().into_iter().filter(|e| e.ready && crate::servers::running(&e.id)) {
                if let Some(g) = crate::servers::game_for(&e.id, || None) {
                    games.push((e.id.clone(), e.name.clone(), g));
                }
            }
            // reports from players (say /report) always notify, whatever the join setting
            for (_, label, g) in &games {
                if let Some(text) = crate::community::report_note(&crate::community::new_reports(&g.data), label) {
                    let _ = app.notification().builder().title("Loner-Strike").body(text).show();
                }
            }
            if crate::app_api::load_settings().mute_joins {
                seen.clear();
                continue;
            }
            let focused = app.get_webview_window("main").and_then(|w| w.is_focused().ok()).unwrap_or(false);
            for (key, label, g) in games {
                let Some(now) = people(&g) else {
                    seen.remove(&key);
                    continue;
                };
                if let Some(before) = seen.get(&key) {
                    let joined: Vec<&String> = now.iter().filter(|p| !before.contains(p)).map(|(_, n)| n).collect();
                    if !joined.is_empty() && !focused {
                        let who = match joined.len() {
                            1 => joined[0].clone(),
                            2 => format!("{} and {}", joined[0], joined[1]),
                            n => format!("{} and {} others", joined[0], n - 1),
                        };
                        let _ = app.notification().builder().title("Loner-Strike").body(format!("{who} joined {label}")).show();
                    }
                }
                seen.insert(key, now);
            }
        }
    });
}
