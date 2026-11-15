//! Playtest screenshots for publishing a mode (agent MCP): the game's own screenshot key (F5,
//! "snapshot") writes `<map>NNNN.bmp` (or .tga) into the game folder while you playtest; the
//! mode's publish form lists the recent ones, turned into PNG, to pick from (at most 6, like a
//! map's).
//!   GET /api/tools/community/playtest-shots   {shots: [{name, map, at, data}], hint}

use crate::server::Shared;
use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// How far back, how many, how big.
const RECENT: Duration = Duration::from_secs(14 * 86400);
const MAX_LISTED: usize = 12;
const MAX_FILE: u64 = 32 << 20;
const MAX_WIDTH: u32 = 1920;

pub const HINT: &str = "While you playtest, press F5 (the game's own screenshot key) to take a shot. They show up here, newest first.";

/// A snapshot's name: `<map>NNNN.bmp` / `.tga` (the map's name and four digits).
pub fn snapshot_map(name: &str) -> Option<String> {
    let l = name.to_ascii_lowercase();
    let stem = l.strip_suffix(".bmp").or_else(|| l.strip_suffix(".tga"))?;
    if stem.len() < 5 {
        return None;
    }
    let (map, digits) = stem.split_at(stem.len() - 4);
    (digits.bytes().all(|c| c.is_ascii_digit()) && crate::maps::safe_arg(map)).then(|| map.to_string())
}

/// The recent snapshots in a game folder (and its screenshots/), newest first.
pub fn recent(game_dir: &Path, now: SystemTime) -> Vec<(PathBuf, String, u64)> {
    let mut v = vec![];
    for dir in [game_dir.to_path_buf(), game_dir.join("screenshots")] {
        for e in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            let Some(map) = snapshot_map(&name) else { continue };
            let Ok(m) = e.metadata() else { continue };
            let Ok(t) = m.modified() else { continue };
            if !m.is_file() || m.len() > MAX_FILE || now.duration_since(t).map_or(false, |age| age > RECENT) {
                continue;
            }
            v.push((e.path(), map, t.duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)));
        }
    }
    v.sort_by(|a, b| b.2.cmp(&a.2));
    v.truncate(MAX_LISTED);
    v
}

/// A BMP or TGA as PNG, at most 1920 wide (a publish screenshot).
pub fn to_png(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let img = image::load_from_memory(bytes).map_err(|_| "That screenshot couldn't be read.".to_string())?;
    let img = if img.width() > MAX_WIDTH { img.resize(MAX_WIDTH, MAX_WIDTH * img.height() / img.width().max(1), image::imageops::FilterType::Triangle) } else { img };
    let mut out = std::io::Cursor::new(Vec::new());
    img.to_rgb8().write_to(&mut out, image::ImageFormat::Png).map_err(|_| "That screenshot couldn't be turned into a PNG.".to_string())?;
    Ok(out.into_inner())
}

fn shots(st: &Shared) -> Result<Value, String> {
    use base64::Engine;
    let game = st.local_game().ok_or("Set up the game on this computer first.")?;
    let mut list = vec![];
    for (p, map, at) in recent(&game.dir, SystemTime::now()) {
        let Ok(bytes) = std::fs::read(&p) else { continue };
        let Ok(png) = to_png(&bytes) else { continue };
        if png.len() > 8 << 20 {
            continue;
        }
        let name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        list.push(json!({"name": name.rsplit_once('.').map(|(s, _)| format!("{s}.png")).unwrap_or(name), "map": map, "at": at,
            "data": format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(&png))}));
    }
    Ok(json!({"shots": list, "hint": HINT}))
}

async fn api_shots(State(st): State<Shared>) -> Json<Value> {
    Json(tokio::task::spawn_blocking(move || shots(&st).unwrap_or_else(|e| json!({"error": e}))).await.unwrap_or_else(|_| json!({"error": "That didn't finish."})))
}

pub fn routes() -> Router<Shared> {
    Router::new().route("/api/tools/community/playtest-shots", get(api_shots))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_names() {
        assert_eq!(snapshot_map("de_dust20003.bmp").as_deref(), Some("de_dust2"));
        assert_eq!(snapshot_map("LS_OUTPOST0012.TGA").as_deref(), Some("ls_outpost"));
        assert_eq!(snapshot_map("de_dust2.bmp"), None);
        assert_eq!(snapshot_map("notes.txt"), None);
        assert_eq!(snapshot_map("a b0001.bmp"), None);
    }

    #[test]
    fn recent_snapshots_become_pngs() {
        let d = std::env::temp_dir().join(format!("ls-shots-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("screenshots")).unwrap();
        // a real 24-bit BMP and a TGA, as the game writes them
        let img = image::RgbImage::from_fn(64, 36, |x, y| image::Rgb([x as u8 * 4, y as u8 * 7, 90]));
        img.save_with_format(d.join("de_dust20001.bmp"), image::ImageFormat::Bmp).unwrap();
        img.save_with_format(d.join("screenshots/ls_outpost0002.tga"), image::ImageFormat::Tga).unwrap();
        std::fs::write(d.join("config.cfg"), "bind F5 snapshot").unwrap();
        let found = recent(&d, SystemTime::now());
        assert_eq!(found.len(), 2);
        let png = to_png(&std::fs::read(&found[0].0).unwrap()).unwrap();
        assert_eq!(image::guess_format(&png).unwrap(), image::ImageFormat::Png);
        // older than two weeks: not listed
        assert!(recent(&d, SystemTime::now() + Duration::from_secs(15 * 86400)).is_empty());
        assert!(to_png(b"not a picture").is_err());
        let _ = std::fs::remove_dir_all(&d);
    }
}
