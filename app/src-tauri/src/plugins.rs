//! The player's own AMX Mod X plugins: a compiled .amxx, or a .sma source that's
//! compiled with the game's own compiler. They go in addons/amxmodx/plugins, load in
//! every mode (a marked block in plugins.ini), and are listed in
//! data/gamemodes/my_plugins.json so they can be taken out again.

use crate::panel_api::from_panel;
use crate::server::Shared;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::Path;
use std::time::Duration;

const BEGIN: &str = "; ---- Loner-Strike: your own plugins (Mods page). Edit them there. ----";
const END: &str = "; ---- end of your own plugins ----";

type Reply = (StatusCode, Json<Value>);

fn safe_name(stem: &str) -> bool {
    !stem.is_empty() && stem.len() <= 48 && stem.chars().all(|c| c.is_ascii_alphanumeric() || "_-.".contains(c)) && !stem.starts_with('.')
}

fn list_path(g: &crate::game::Game) -> std::path::PathBuf {
    g.data.join("my_plugins.json")
}

pub fn load(g: &crate::game::Game) -> Vec<String> {
    std::fs::read_to_string(list_path(g)).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

/// plugins.ini with our block listing `names` (replaced in place, or added at the end).
pub fn rewrite_ini(text: &str, names: &[String]) -> String {
    let nl = if text.contains("\r\n") || text.is_empty() { "\r\n" } else { "\n" };
    let mut block = vec![BEGIN.to_string()];
    block.extend(names.iter().map(|n| format!("{n}.amxx")));
    block.push(END.to_string());
    let block = block.join(nl);
    if let (Some(s), Some(e)) = (text.find(BEGIN), text.find(END)) {
        if e > s {
            return format!("{}{block}{}", &text[..s], &text[e + END.len()..]);
        }
    }
    let base = text.trim_end_matches(['\r', '\n']);
    if base.is_empty() {
        format!("{block}{nl}")
    } else {
        format!("{base}{nl}{nl}{block}{nl}")
    }
}

fn save(g: &crate::game::Game, names: &[String]) -> Result<(), String> {
    crate::sys::write_atomic(&list_path(g), serde_json::to_string_pretty(names).unwrap_or_default().as_bytes()).map_err(|e| e.to_string())?;
    let ini = g.dir.join("addons/amxmodx/configs/plugins.ini");
    let old = crate::sys::read_latin1(&ini).unwrap_or_default();
    crate::sys::write_latin1(&ini, &rewrite_ini(&old, names)).map_err(|e| format!("Couldn't update plugins.ini ({e})."))
}

/// The install a game folder belongs to (for running the compiler in its bottle).
fn install_of(g: &crate::game::Game) -> Option<crate::steam::Install> {
    crate::steam::find_installs().into_iter().find(|i| i.hl == g.hl).or_else(|| crate::servers::find_install(&g.hl))
}

/// Copies (or compiles) a picked plugin into the game. Returns its name.
pub fn add(g: &crate::game::Game, file: &Path) -> Result<String, String> {
    let am = g.dir.join("addons/amxmodx");
    if !am.join("configs").is_dir() {
        return Err("AMX Mod X isn't installed here yet. Install the basics first.".into());
    }
    let fname = file.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let (stem, ext) = fname.rsplit_once('.').map(|(a, b)| (a.to_string(), b.to_lowercase())).unwrap_or_default();
    if !safe_name(&stem) {
        return Err("Rename the file to use only letters, numbers, - and _ first.".into());
    }
    let mine = load(g);
    let out = am.join("plugins").join(format!("{stem}.amxx"));
    if out.exists() && !mine.contains(&stem) {
        return Err(format!("There's already a plugin called {stem}.amxx that you didn't add here. Rename yours to add it."));
    }
    match ext.as_str() {
        "amxx" => {
            std::fs::copy(file, &out).map_err(|e| format!("Couldn't copy it ({e})."))?;
        }
        "sma" => {
            let src = am.join("scripting").join(format!("{stem}.sma"));
            std::fs::copy(file, &src).map_err(|e| format!("Couldn't copy it ({e})."))?;
            let install = install_of(g).ok_or("Couldn't find this game's install to compile with.")?;
            if install.kind == "crossover" && !Path::new("/Applications/CrossOver.app/Contents/SharedSupport/CrossOver/bin/wine").is_file() {
                return Err("CrossOver isn't installed in Applications, so the plugin can't be compiled. Add its .amxx instead.".into());
            }
            let mut cmd = crate::setup::core::windows_command(&install, &am.join("scripting/amxxpc.exe"));
            cmd.arg(&src).arg(format!("-o{}", out.display()));
            let (_, o, e) = crate::sys::run_output(cmd, Duration::from_secs(120)).ok_or("The compiler didn't run.")?;
            let all = format!("{o}{e}");
            if !all.contains("Done.") || !out.is_file() {
                let errors: Vec<&str> = all.lines().filter(|l| l.to_lowercase().contains("error")).take(4).collect();
                return Err(format!("It didn't compile: {}", if errors.is_empty() { "the compiler gave no reason".into() } else { errors.join(" / ") }));
            }
        }
        _ => return Err("Pick an AMX Mod X plugin: a .amxx file, or its .sma source.".into()),
    }
    let mut names = mine;
    if !names.contains(&stem) {
        names.push(stem.clone());
    }
    save(g, &names)?;
    Ok(stem)
}

pub fn remove(g: &crate::game::Game, name: &str) -> Result<(), String> {
    let mut names = load(g);
    if !names.iter().any(|n| n == name) {
        return Err("That plugin isn't one you added.".into());
    }
    names.retain(|n| n != name);
    save(g, &names)?;
    let am = g.dir.join("addons/amxmodx");
    let _ = std::fs::remove_file(am.join("plugins").join(format!("{name}.amxx")));
    let _ = std::fs::remove_file(am.join("scripting").join(format!("{name}.sma")));
    Ok(())
}

async fn api_list(State(st): State<Shared>) -> Json<Value> {
    Json(match st.game() {
        Some(g) => json!({"plugins": load(&g)}),
        None => json!({"plugins": []}),
    })
}

async fn api_add(State(st): State<Shared>, headers: HeaderMap) -> Reply {
    use tauri_plugin_dialog::DialogExt;
    if !from_panel(&headers) {
        return (StatusCode::FORBIDDEN, Json(json!({"error": "forbidden"})));
    }
    let (Some(g), Some(app)) = (st.game(), st.app.get().cloned()) else {
        return (StatusCode::OK, Json(json!({"error": "No game found."})));
    };
    let r = tokio::task::spawn_blocking(move || {
        let Some(path) = app.dialog().file().set_title("Add an AMX Mod X plugin").add_filter("AMX Mod X plugin", &["amxx", "sma"]).blocking_pick_file() else {
            return Ok(json!({"cancelled": true}));
        };
        let path = path.into_path().map_err(|e| e.to_string())?;
        add(&g, &path).map(|name| json!({"ok": true, "name": name, "plugins": load(&g)}))
    })
    .await
    .unwrap_or_else(|_| Err("Something went wrong.".into()));
    (StatusCode::OK, Json(r.unwrap_or_else(|e| json!({"error": e}))))
}

#[derive(Deserialize)]
struct RemoveReq {
    name: String,
}

async fn api_remove(State(st): State<Shared>, headers: HeaderMap, Json(req): Json<RemoveReq>) -> Reply {
    if !from_panel(&headers) {
        return (StatusCode::FORBIDDEN, Json(json!({"error": "forbidden"})));
    }
    let Some(g) = st.game() else { return (StatusCode::OK, Json(json!({"error": "No game found."}))) };
    let r = remove(&g, req.name.trim()).map(|_| json!({"ok": true, "plugins": load(&g)}));
    (StatusCode::OK, Json(r.unwrap_or_else(|e| json!({"error": e}))))
}

pub fn routes() -> Router<Shared> {
    Router::new().route("/api/mods/plugins", get(api_list))
}

/// The app window only: adding opens a file picker, and removing changes the game's files.
pub fn local_routes() -> Router<Shared> {
    Router::new().route("/api/mods/plugins/add", post(api_add)).route("/api/mods/plugins/remove", post(api_remove))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ini_block() {
        let ini = "; AMX Mod X plugins\r\nadmin.amxx\r\n";
        let once = rewrite_ini(ini, &["myplugin".into()]);
        assert!(once.contains("admin.amxx") && once.contains("\r\nmyplugin.amxx\r\n"));
        let twice = rewrite_ini(&once, &["a".into(), "b".into()]);
        assert_eq!(twice.matches(BEGIN).count(), 1);
        assert!(!twice.contains("myplugin") && twice.contains("a.amxx") && twice.contains("b.amxx"));
        assert!(safe_name("my_plugin-2") && !safe_name("../x") && !safe_name("a b") && !safe_name(""));
    }
}
