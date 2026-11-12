//! The spray maker: a picture from this computer becomes the player's spray in
//! Counter-Strike 1.6 and Condition Zero (`tempdecal.wad` in cstrike/ and czero/).
//!
//! The page never names a path: the picture comes from the system's file dialog and is
//! kept here until the next one. The game rewrites tempdecal.wad when the player picks a
//! spray in Options > Multiplayer, so (like the community does) the file is made read-only
//! unless the player says not to. The file it replaces is kept in the app's folder
//! (sprays/backups) and "Put back" brings it back.
//!
//! App window only (merged into tools::local_routes). Errors are `{error}` with status 200,
//! like the rest of the app; POSTs need the panel header.

use crate::goldsrc::wadwrite::{self, Clear, Look, Options, Spray};
use crate::server::Shared;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use base64::Engine;
use image::RgbaImage;
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

const NO_GAME: &str = "Set up the game on this computer first.";
const NO_PICTURE: &str = "Choose a picture first.";
const INTERNAL: &str = "Loner-Strike hit an internal error making the spray. Try again, and copy the diagnostics from Settings if it keeps happening.";
/// The biggest picture file we open, and its biggest side.
const MAX_FILE: u64 = 40 << 20;
const MAX_SIDE: u32 = 8192;
/// Backups kept per game.
const KEEP: usize = 10;
pub const FILE: &str = "tempdecal.wad";
pub const GAMES: [(&str, &str); 2] = [("cstrike", "Counter-Strike 1.6"), ("czero", "Condition Zero")];

/// The picture the player chose (its file name and pixels).
struct Picked {
    name: String,
    img: RgbaImage,
}

static PICKED: Mutex<Option<Picked>> = Mutex::new(None);

fn picked<T>(f: impl FnOnce(&Picked) -> T) -> Option<T> {
    PICKED.lock().unwrap_or_else(|e| e.into_inner()).as_ref().map(f)
}

/// Opens a picture file (PNG, JPEG, BMP, TGA, WebP), refusing huge ones.
pub fn load_picture(path: &Path) -> Result<RgbaImage, String> {
    let size = std::fs::metadata(path).map_err(|e| format!("Couldn't open that file: {e}"))?.len();
    if size > MAX_FILE {
        return Err(format!("That picture is too big ({} MB). Pick one under {} MB.", size >> 20, MAX_FILE >> 20));
    }
    let mut r = image::ImageReader::open(path).map_err(|e| format!("Couldn't open that file: {e}"))?.with_guessed_format().map_err(|e| format!("Couldn't open that file: {e}"))?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(MAX_SIDE);
    limits.max_image_height = Some(MAX_SIDE);
    r.limits(limits);
    if r.format().is_none() {
        return Err("That isn't a picture Loner-Strike can read. Use a PNG, JPEG, BMP, TGA or WebP file.".into());
    }
    let img = r.decode().map_err(|e| match e {
        image::ImageError::Limits(_) => format!("That picture is too big. Pick one under {MAX_SIDE}x{MAX_SIDE} pixels."),
        image::ImageError::Unsupported(_) => "That isn't a picture Loner-Strike can read. Use a PNG, JPEG, BMP, TGA or WebP file.".into(),
        _ => "That picture looks damaged. Try saving it again, or pick another.".into(),
    })?;
    Ok(img.to_rgba8())
}

/// The spray choices from the page, with sensible defaults for anything left out.
#[derive(Debug, Deserialize, Default, Clone)]
#[serde(default)]
pub struct Choice {
    /// "small" | "medium" | "large" (default)
    pub size: String,
    /// "colour" (default) | "mono"
    pub look: String,
    /// The paint colour for "mono", "#rrggbb" (default orange, like the game's).
    pub paint: String,
    pub invert: bool,
    /// "alpha" (default) | "background" | "none"
    pub clear: String,
    pub dither: bool,
}

impl Choice {
    pub fn options(&self) -> Options {
        let max_pixels = match self.size.as_str() {
            "small" => 4096,
            "medium" => 8192,
            _ => wadwrite::MAX_PIXELS,
        };
        let look = match self.look.as_str() {
            "mono" => Look::Mono { paint: hex_colour(&self.paint).unwrap_or([255, 120, 24]), invert: self.invert },
            _ => Look::Colour,
        };
        let clear = match self.clear.as_str() {
            "background" => Clear::Background,
            "none" => Clear::None,
            _ => Clear::Alpha,
        };
        Options { max_pixels, look, clear, dither: self.dither }
    }
}

fn hex_colour(s: &str) -> Option<[u8; 3]> {
    let h = s.strip_prefix('#')?;
    if h.len() != 6 || !h.bytes().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let v = u32::from_str_radix(h, 16).ok()?;
    Some([(v >> 16) as u8, (v >> 8) as u8, v as u8])
}

fn png_data_url(s: &Spray) -> Result<String, String> {
    let png = crate::goldsrc::png(s.width, s.height, &s.to_rgba(), true).map_err(|e| e.to_string())?;
    Ok(format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(png)))
}

/// What the page shows for a spray: the picture as the game draws it, its size and colours.
fn describe(s: &Spray) -> Result<Value, String> {
    Ok(json!({
        "png": png_data_url(s)?,
        "width": s.width,
        "height": s.height,
        "colour": s.is_colour(),
        "colours": s.colours_used(),
    }))
}

// ---- files -----------------------------------------------------------------------------

/// The game folders a spray goes in (those that exist), as (folder name, label, path).
pub fn game_dirs(hl: &Path) -> Vec<(&'static str, &'static str, PathBuf)> {
    GAMES.iter().map(|&(g, label)| (g, label, hl.join(g))).filter(|(_, _, d)| d.is_dir()).collect()
}

fn backup_dir(app: &Path) -> PathBuf {
    app.join("sprays").join("backups")
}

/// Backups for one game, newest first.
fn backups(app: &Path, game: &str) -> Vec<PathBuf> {
    let prefix = format!("{game}-");
    let mut v: Vec<PathBuf> = std::fs::read_dir(backup_dir(app))
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with(&prefix) && n.ends_with(".wad")))
        .collect();
    // names carry a zero-padded time, so the name order is the time order
    v.sort();
    v.reverse();
    v
}

fn is_read_only(p: &Path) -> bool {
    std::fs::metadata(p).is_ok_and(|m| m.permissions().readonly())
}

fn set_read_only(p: &Path, on: bool) -> std::io::Result<()> {
    let mut perm = std::fs::metadata(p)?.permissions();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        perm.set_mode(if on { 0o444 } else { 0o644 });
    }
    #[cfg(not(unix))]
    perm.set_readonly(on);
    std::fs::set_permissions(p, perm)
}

/// Writes tempdecal.wad in a game folder: the old one kept (when `backup`), the new one
/// written beside it and moved over, then made read-only when `lock`.
fn put(app: &Path, game: &str, dir: &Path, bytes: &[u8], lock: bool, backup: bool) -> Result<(), String> {
    let target = dir.join(FILE);
    let words = |e: std::io::Error| match e.kind() {
        std::io::ErrorKind::PermissionDenied => format!("Loner-Strike isn't allowed to change files in the {game} folder."),
        _ => format!("Couldn't save the spray in the {game} folder: {e}"),
    };
    if target.is_file() {
        if backup {
            let b = backup_dir(app);
            std::fs::create_dir_all(&b).map_err(|e| format!("Couldn't keep a copy of your old spray: {e}"))?;
            let ms = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
            std::fs::copy(&target, b.join(format!("{game}-{ms:015}.wad"))).map_err(|e| format!("Couldn't keep a copy of your old spray: {e}"))?;
            for old in backups(app, game).into_iter().skip(KEEP) {
                let _ = std::fs::remove_file(old);
            }
        }
        if is_read_only(&target) {
            set_read_only(&target, false).map_err(words)?;
        }
    }
    let tmp = dir.join(format!("{FILE}.new"));
    std::fs::write(&tmp, bytes).map_err(words)?;
    if let Err(e) = std::fs::rename(&tmp, &target) {
        let _ = std::fs::remove_file(&tmp);
        return Err(words(e));
    }
    if lock {
        set_read_only(&target, true).map_err(words)?;
    }
    Ok(())
}

/// The spray now in each game folder, and whether there's one to put back.
pub fn status(hl: &Path, app: &Path) -> Value {
    let games: Vec<Value> = game_dirs(hl)
        .into_iter()
        .map(|(g, label, dir)| {
            let p = dir.join(FILE);
            let current = std::fs::read(&p).ok().map(|b| match wadwrite::read_wad(&b).map_err(|e| e.to_string()).and_then(|s| describe(&s)) {
                Ok(v) => v,
                Err(e) => json!({"error": e}),
            });
            json!({
                "game": g,
                "label": label,
                "current": current,
                "readOnly": p.is_file() && is_read_only(&p),
                "canRestore": !backups(app, g).is_empty(),
            })
        })
        .collect();
    json!({
        "games": games,
        "picked": picked(|p| json!({"name": p.name, "width": p.img.width(), "height": p.img.height()})),
    })
}

/// Makes the spray from the chosen picture and installs it in `games` (empty: every game
/// folder there is). Returns the success words.
pub fn install(hl: &Path, app: &Path, img: &RgbaImage, choice: &Choice, games: &[String], lock: bool) -> Result<Value, String> {
    let spray = wadwrite::make(img, &choice.options());
    let bytes = wadwrite::write_wad(&spray).map_err(|e| e.to_string())?;
    let dirs: Vec<_> = game_dirs(hl).into_iter().filter(|(g, ..)| games.is_empty() || games.iter().any(|w| w == g)).collect();
    if dirs.is_empty() {
        return Err("Couldn't find Counter-Strike 1.6 or Condition Zero on this computer.".into());
    }
    let mut done = Vec::new();
    for (g, label, dir) in dirs {
        put(app, g, &dir, &bytes, lock, true)?;
        done.push(label);
    }
    Ok(json!({"ok": true, "installed": done, "message": "Spray installed. Press T in game to spray it."}))
}

/// Puts the newest kept spray back in one game folder (and forgets that copy, so doing it
/// again goes one further back).
pub fn restore(hl: &Path, app: &Path, game: &str) -> Result<Value, String> {
    let Some((g, label, dir)) = game_dirs(hl).into_iter().find(|(g, ..)| *g == game) else {
        return Err("That game isn't on this computer.".into());
    };
    let Some(newest) = backups(app, g).into_iter().next() else {
        return Err(format!("There's no earlier spray kept for {label}."));
    };
    let bytes = std::fs::read(&newest).map_err(|e| format!("Couldn't read the copy of your old spray: {e}"))?;
    let lock = is_read_only(&dir.join(FILE));
    put(app, g, &dir, &bytes, lock, false)?;
    let _ = std::fs::remove_file(&newest);
    Ok(json!({"ok": true, "message": format!("Your previous spray is back in {label}.")}))
}

// ---- routes ----------------------------------------------------------------------------

fn answer(r: Result<Value, String>) -> Response {
    match r {
        Ok(v) => Json(v).into_response(),
        Err(e) => Json(json!({"error": e})).into_response(),
    }
}

fn forbidden() -> Response {
    (StatusCode::FORBIDDEN, Json(json!({"error": "forbidden"}))).into_response()
}

async fn blocking(f: impl FnOnce() -> Result<Value, String> + Send + 'static) -> Response {
    answer(tokio::task::spawn_blocking(f).await.unwrap_or_else(|_| Err(INTERNAL.into())))
}

fn folders(st: &Shared) -> Result<(PathBuf, PathBuf), String> {
    let g = st.local_game().ok_or(NO_GAME)?;
    Ok((g.hl.clone(), g.app.clone()))
}

async fn api_status(State(st): State<Shared>) -> Response {
    let (hl, app) = match folders(&st) {
        Ok(f) => f,
        Err(e) => return answer(Err(e)),
    };
    blocking(move || Ok(status(&hl, &app))).await
}

async fn api_pick(State(st): State<Shared>, h: HeaderMap) -> Response {
    use tauri_plugin_dialog::DialogExt;
    if !crate::panel_api::from_panel(&h) {
        return forbidden();
    }
    let Some(app) = st.app.get().cloned() else { return answer(Err(INTERNAL.into())) };
    blocking(move || {
        let Some(path) = app.dialog().file().set_title("Choose a picture for your spray").add_filter("Pictures", &["png", "jpg", "jpeg", "bmp", "tga", "webp"]).blocking_pick_file() else {
            return Ok(json!({"cancelled": true}));
        };
        let path = path.into_path().map_err(|e| e.to_string())?;
        let img = load_picture(&path)?;
        let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        let v = json!({"name": name, "width": img.width(), "height": img.height()});
        *PICKED.lock().unwrap_or_else(|e| e.into_inner()) = Some(Picked { name, img });
        Ok(v)
    })
    .await
}

async fn api_preview(h: HeaderMap, Json(c): Json<Choice>) -> Response {
    if !crate::panel_api::from_panel(&h) {
        return forbidden();
    }
    blocking(move || {
        let img = picked(|p| p.img.clone()).ok_or(NO_PICTURE)?;
        describe(&wadwrite::make(&img, &c.options()))
    })
    .await
}

#[derive(Deserialize)]
struct InstallReq {
    #[serde(default)]
    choice: Choice,
    #[serde(default)]
    games: Vec<String>,
    #[serde(default = "yes")]
    lock: bool,
}

fn yes() -> bool {
    true
}

async fn api_install(State(st): State<Shared>, h: HeaderMap, Json(req): Json<InstallReq>) -> Response {
    if !crate::panel_api::from_panel(&h) {
        return forbidden();
    }
    let (hl, app) = match folders(&st) {
        Ok(f) => f,
        Err(e) => return answer(Err(e)),
    };
    blocking(move || {
        let img = picked(|p| p.img.clone()).ok_or(NO_PICTURE)?;
        install(&hl, &app, &img, &req.choice, &req.games, req.lock)
    })
    .await
}

#[derive(Deserialize)]
struct RestoreReq {
    game: String,
}

async fn api_restore(State(st): State<Shared>, h: HeaderMap, Json(req): Json<RestoreReq>) -> Response {
    if !crate::panel_api::from_panel(&h) {
        return forbidden();
    }
    let (hl, app) = match folders(&st) {
        Ok(f) => f,
        Err(e) => return answer(Err(e)),
    };
    blocking(move || restore(&hl, &app, &req.game)).await
}

/// The app window's listener only (merged into tools::local_routes).
pub fn routes() -> Router<Shared> {
    Router::new()
        .route("/api/tools/spray", get(api_status))
        .route("/api/tools/spray/pick", post(api_pick))
        .route("/api/tools/spray/preview", post(api_preview))
        .route("/api/tools/spray/install", post(api_install))
        .route("/api/tools/spray/restore", post(api_restore))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(tag: &str) -> (PathBuf, PathBuf, PathBuf) {
        let root = std::env::temp_dir().join(format!("ls-spray-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let hl = root.join("Half-Life");
        std::fs::create_dir_all(hl.join("cstrike")).unwrap();
        std::fs::create_dir_all(hl.join("czero")).unwrap();
        (root.clone(), hl, root.join("app"))
    }

    fn photo() -> RgbaImage {
        RgbaImage::from_fn(300, 200, |x, y| image::Rgba([(x % 256) as u8, (y % 256) as u8, 120, if x < 20 { 0 } else { 255 }]))
    }

    #[test]
    fn choices_have_defaults() {
        let o = Choice::default().options();
        assert_eq!(o, Options { max_pixels: wadwrite::MAX_PIXELS, look: Look::Colour, clear: Clear::Alpha, dither: false });
        let c: Choice = serde_json::from_value(json!({"size": "small", "look": "mono", "paint": "#00ff80", "clear": "background"})).unwrap();
        let o = c.options();
        assert_eq!(o.max_pixels, 4096);
        assert_eq!(o.look, Look::Mono { paint: [0, 255, 128], invert: false });
        assert_eq!(o.clear, Clear::Background);
        assert_eq!(hex_colour("#zzzzzz"), None);
        assert_eq!(hex_colour("ff0000"), None);
    }

    #[test]
    fn install_backup_lock_and_restore() {
        let (root, hl, app) = temp("install");
        // the game's own spray is there already, and read-only
        let old = b"old spray".to_vec();
        std::fs::write(hl.join("cstrike").join(FILE), &old).unwrap();
        set_read_only(&hl.join("cstrike").join(FILE), true).unwrap();

        let v = install(&hl, &app, &photo(), &Choice::default(), &[], true).unwrap();
        assert_eq!(v["message"], "Spray installed. Press T in game to spray it.");
        assert_eq!(v["installed"], json!(["Counter-Strike 1.6", "Condition Zero"]));
        for g in ["cstrike", "czero"] {
            let p = hl.join(g).join(FILE);
            let s = wadwrite::read_wad(&std::fs::read(&p).unwrap()).unwrap();
            assert!(s.is_colour() && wadwrite::valid_size(s.width, s.height));
            assert!(is_read_only(&p), "{g} not read-only");
            assert!(!hl.join(g).join(format!("{FILE}.new")).exists());
        }
        // the old one was kept (czero had none)
        assert_eq!(backups(&app, "cstrike").len(), 1);
        assert!(backups(&app, "czero").is_empty());

        let st = status(&hl, &app);
        assert_eq!(st["games"][0]["readOnly"], true);
        assert_eq!(st["games"][0]["canRestore"], true);
        assert!(st["games"][0]["current"]["png"].as_str().unwrap().starts_with("data:image/png;base64,"));
        assert_eq!(st["games"][1]["canRestore"], false);

        // only one game, not locked
        let mono: Choice = serde_json::from_value(json!({"look": "mono"})).unwrap();
        install(&hl, &app, &photo(), &mono, &["czero".into()], false).unwrap();
        assert!(!is_read_only(&hl.join("czero").join(FILE)));
        assert!(!wadwrite::read_wad(&std::fs::read(hl.join("czero").join(FILE)).unwrap()).unwrap().is_colour());
        assert_eq!(backups(&app, "czero").len(), 1);

        // put back: the game's file returns, still read-only, and the copy is used up
        let v = restore(&hl, &app, "cstrike").unwrap();
        assert_eq!(v["ok"], true);
        assert_eq!(std::fs::read(hl.join("cstrike").join(FILE)).unwrap(), old);
        assert!(is_read_only(&hl.join("cstrike").join(FILE)));
        assert!(restore(&hl, &app, "cstrike").unwrap_err().contains("no earlier spray"));
        assert!(restore(&hl, &app, "valve").is_err());
        // a damaged file shows as an error in the status, not a failure
        let st = status(&hl, &app);
        assert!(st["games"][0]["current"]["error"].is_string());

        for g in ["cstrike", "czero"] {
            let _ = set_read_only(&hl.join(g).join(FILE), false);
        }
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn backups_are_capped() {
        let (root, hl, app) = temp("cap");
        for _ in 0..(KEEP + 3) {
            install(&hl, &app, &photo(), &Choice::default(), &["cstrike".into()], false).unwrap();
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        assert_eq!(backups(&app, "cstrike").len(), KEEP);
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn no_game_folders() {
        let root = std::env::temp_dir().join(format!("ls-spray-none-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let e = install(&root, &root.join("app"), &photo(), &Choice::default(), &[], true).unwrap_err();
        assert!(e.contains("Couldn't find"), "{e}");
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn pictures_load_or_say_why() {
        let (root, ..) = temp("load");
        let png = root.join("a.png");
        std::fs::write(&png, crate::goldsrc::png(2, 2, &[255; 16], true).unwrap()).unwrap();
        assert_eq!(load_picture(&png).unwrap().dimensions(), (2, 2));
        let txt = root.join("b.png");
        std::fs::write(&txt, b"not a picture at all").unwrap();
        let e = load_picture(&txt).unwrap_err();
        assert!(e.contains("isn't a picture") || e.contains("damaged"), "{e}");
        assert!(load_picture(&root.join("missing.png")).unwrap_err().starts_with("Couldn't open"));
        std::fs::remove_dir_all(&root).ok();
    }
}
