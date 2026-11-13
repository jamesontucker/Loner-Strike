//! What the map viewer needs to draw a map the way the game does (docs/TOOLS-RENDER.md):
//!
//! - `GET /api/tools/map/:name/look`       the player's own picture settings (gamma, brightness,
//!   texgamma, lightgamma, gl_overbright, texture filtering) from the game's config.cfg, the
//!   sky's name and which of its six pictures are here, the sun (light_environment) as the
//!   game hands it to models, and the light at every model-carrying entity (for shading it)
//! - `GET /api/tools/map/:name/sky/:side`  one sky picture (rt bk lf ft up dn) as PNG, found
//!   like the game finds it: gfx/env/<skyname><side>.tga in the game's folders
//!
//! Nothing here takes a path from the page: the sky's name comes from the map itself.

use super::mapinfo::{self, MapFile};
use super::{binary_answer, blocking, find_map, json_answer, locate, Cached, Fail};
use crate::goldsrc::{self, bsp::Bsp, entities::Entity, lightmap};
use crate::mapcopy::Finder;
use crate::server::Shared;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::Response;
use axum::routing::get;
use axum::Router;
use serde_json::{json, Value};
use std::path::PathBuf;

pub const SIDES: [&str; 6] = ["rt", "bk", "lf", "ft", "up", "dn"];
/// A sky picture bigger than this (per side, or as a file) isn't one.
const MAX_SKY_SIDE: u32 = 2048;
const MAX_SKY_FILE: u64 = 32 << 20;
/// The light-sampling work for models is per entity; a map with absurdly many is cut off.
const MAX_LIT: usize = 512;

// ---- the player's picture settings ------------------------------------------------------

/// The settings that change how the game draws a map. Defaults are the game's own.
#[derive(Debug, Clone, PartialEq)]
pub struct Settings {
    pub gamma: f32,
    pub brightness: f32,
    pub texgamma: f32,
    pub lightgamma: f32,
    pub overbright: bool,
    pub texturemode: String,
    pub anisotropy: f32,
    pub detail: bool,
}

impl Default for Settings {
    fn default() -> Self {
        // brightness: the game's video options write 1 on a new install (all three config.cfg
        // files here say "1.000000"); the engine clamps gamma/texgamma/lightgamma to 1.8-3
        Settings { gamma: 2.5, brightness: 1.0, texgamma: 2.0, lightgamma: 2.5, overbright: false, texturemode: "GL_LINEAR_MIPMAP_LINEAR".into(), anisotropy: 16.0, detail: false }
    }
}

/// Reads `name "value"` lines (later ones win, as when the game runs them in order).
pub fn read_settings(texts: &[String]) -> Settings {
    let mut s = Settings::default();
    for text in texts {
        for line in text.lines() {
            let line = line.split("//").next().unwrap_or("").trim();
            let mut parts = line.splitn(2, char::is_whitespace);
            let (Some(k), Some(v)) = (parts.next(), parts.next()) else { continue };
            let v = v.trim().trim_matches('"').trim();
            let num = v.parse::<f32>().ok().filter(|f| f.is_finite());
            match (k.to_ascii_lowercase().as_str(), num) {
                ("gamma", Some(f)) => s.gamma = f.clamp(1.8, 3.0),
                ("brightness", Some(f)) => s.brightness = f.clamp(0.0, 3.0),
                ("texgamma", Some(f)) => s.texgamma = f.clamp(1.8, 3.0),
                ("lightgamma", Some(f)) => s.lightgamma = f.clamp(1.8, 3.0),
                ("gl_overbright", Some(f)) => s.overbright = f != 0.0,
                ("gl_ansio", Some(f)) => s.anisotropy = f.clamp(1.0, 16.0),
                ("r_detailtextures", Some(f)) => s.detail = f != 0.0,
                ("gl_texturemode", _) if v.starts_with("GL_") && v.len() < 32 => s.texturemode = v.to_ascii_uppercase(),
                _ => {}
            }
        }
    }
    s
}

/// The config files the game runs for this map's game folder, in order.
fn config_texts(m: &MapFile) -> (Vec<String>, Vec<String>) {
    let f = Finder::default();
    let dir = m.hl.join(&m.game);
    let (mut texts, mut from) = (Vec::new(), Vec::new());
    for name in ["config.cfg", "userconfig.cfg", "autoexec.cfg"] {
        let Some(p) = f.find(&dir, name) else { continue };
        if std::fs::metadata(&p).map(|md| md.len() > 1 << 20).unwrap_or(true) {
            continue;
        }
        if let Ok(b) = std::fs::read(&p) {
            texts.push(crate::rcon::latin1(&b));
            from.push(format!("{}/{name}", m.game));
        }
    }
    (texts, from)
}

// ---- sky --------------------------------------------------------------------------------

/// A sky name as maps write it: one plain part (letters, digits, `_ - .`), not too long.
pub fn safe_sky(name: &str) -> bool {
    !name.is_empty() && name.len() <= 48 && !name.starts_with('.') && name.bytes().all(|c| c.is_ascii_alphanumeric() || b"_-.".contains(&c))
}

fn sky_name(entities: &[Entity]) -> Option<String> {
    let w = entities.iter().find(|e| e.classname() == "worldspawn")?;
    // the game's default when a map names none
    let n = w.get("skyname").map(str::trim).filter(|s| !s.is_empty()).unwrap_or("desert");
    safe_sky(n).then(|| n.to_string())
}

/// gfx/env/<sky><side>.tga, first in the game's own folders (whatever the case of the name).
fn sky_file(m: &MapFile, sky: &str, side: &str) -> Option<PathBuf> {
    let f = Finder::default();
    for dir in goldsrc::wad::search_dirs(&m.hl, m.czero) {
        for ext in ["tga", "bmp"] {
            if let Some(p) = f.find(&dir, &format!("gfx/env/{sky}{side}.{ext}")) {
                return Some(p);
            }
        }
    }
    None
}

fn sky_png(path: &std::path::Path) -> Result<Vec<u8>, Fail> {
    let damaged = || Fail::new(StatusCode::UNPROCESSABLE_ENTITY, "That sky picture is damaged.");
    if std::fs::metadata(path).map(|md| md.len() > MAX_SKY_FILE).unwrap_or(true) {
        return Err(damaged());
    }
    let bytes = std::fs::read(path).map_err(|_| Fail::new(StatusCode::NOT_FOUND, "That sky picture isn't on this computer."))?;
    let fmt = if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("bmp")) { image::ImageFormat::Bmp } else { image::ImageFormat::Tga };
    let mut reader = image::ImageReader::with_format(std::io::Cursor::new(bytes), fmt);
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(MAX_SKY_SIDE);
    limits.max_image_height = Some(MAX_SKY_SIDE);
    limits.max_alloc = Some(64 << 20);
    reader.limits(limits);
    let img = reader.decode().map_err(|_| damaged())?.to_rgb8();
    goldsrc::png(img.width(), img.height(), img.as_raw(), false).map_err(Fail::from)
}

// ---- the sun and light for models -------------------------------------------------------

/// The sun as the game DLL hands it to the engine from light_environment (`sv_skycolor`,
/// `sv_skyvec`): colour `pow(c / 114, 0.6) * 264` per channel (after the 4th number scales
/// it), direction its forward vector with the pitch turned over (it points the way the light
/// travels: down for pitch -90). None without a light_environment.
pub fn sun(entities: &[Entity]) -> Option<([i32; 3], [f32; 3])> {
    let e = entities.iter().find(|e| e.classname() == "light_environment")?;
    let nums: Vec<i32> = e.get("_light").unwrap_or("").split_whitespace().map_while(|s| s.parse::<i32>().ok()).take(4).collect();
    let (mut r, mut g, mut b) = match nums.len() {
        0 => return None,
        1 | 2 => (nums[0], nums[0], nums[0]),
        _ => (nums[0], nums[1], nums[2]),
    };
    if nums.len() == 4 {
        let v = nums[3] as f64 / 255.0;
        (r, g, b) = ((r as f64 * v) as i32, (g as f64 * v) as i32, (b as f64 * v) as i32);
    }
    let scale = |c: i32| ((c.max(0) as f64 / 114.0).powf(0.6) * 264.0) as i32;
    let colour = [scale(r), scale(g), scale(b)];
    // angles in the order the keys come ("angle" is turned into "angles" by the engine)
    let mut ang = [0f32; 3];
    for (k, v) in &e.keys {
        let n = |s: &str| s.trim().parse::<f32>().ok().filter(|f| f.is_finite());
        match k.as_str() {
            "angles" => {
                let p: Vec<f32> = v.split_whitespace().filter_map(n).collect();
                if p.len() == 3 {
                    ang = [p[0], p[1], p[2]];
                }
            }
            "angle" => match n(v) {
                Some(a) if a == -1.0 => ang = [-90.0, 0.0, 0.0],
                Some(a) if a == -2.0 => ang = [90.0, 0.0, 0.0],
                Some(a) => ang = [0.0, a, 0.0],
                None => {}
            },
            "pitch" => {
                if let Some(p) = n(v) {
                    ang[0] = p;
                }
            }
            _ => {}
        }
    }
    let (p, y) = ((-ang[0]).to_radians(), ang[1].to_radians());
    let dir = [p.cos() * y.cos(), p.cos() * y.sin(), -p.sin()];
    Some((colour, dir))
}

/// The light a model standing at `origin` gets, as the engine works it out: straight down
/// from 8 units above; if the sun is set and a line towards it meets sky first, the sun's
/// colour from the sun's direction; else the floor's light, with a direction from how the
/// floor's light changes around the spot (four samples 16 units away). Colours 0-255 (the
/// sun's may be more); `dir` is the way the light travels.
pub fn model_light(bsp: &Bsp, origin: [f32; 3], sun: Option<([i32; 3], [f32; 3])>) -> ([f32; 3], [f32; 3], bool) {
    let src = [origin[0], origin[1], origin[2] + 8.0];
    if let Some((c, v)) = sun.filter(|(c, _)| c.iter().sum::<i32>() != 0) {
        let end = [origin[0] - v[0] * 8192.0, origin[1] - v[1] * 8192.0, origin[2] - v[2] * 8192.0];
        if let Some(w) = bsp.models.first() {
            let faces = (w.first_face.max(0) as usize)..((w.first_face.max(0) + w.num_faces.max(0)) as usize).min(bsp.faces.len());
            // what the line meets first, passing through water and other non-solid surfaces
            let see_through = |fi: usize| bsp.faces.get(fi).and_then(|f| bsp.face_texture(f)).map_or(true, |(_, t)| {
                let n = bsp.textures[t].name.to_ascii_lowercase();
                n.starts_with('!') || n.starts_with('*') || n.starts_with("water") || n == "aaatrigger"
            });
            if let Some((fi, _, _)) = lightmap::first_hit(bsp, faces, src, end, see_through) {
                let is_sky = bsp.face_texture(&bsp.faces[fi]).is_some_and(|(_, t)| bsp.textures[t].name.eq_ignore_ascii_case("sky"));
                if is_sky {
                    return (c.map(|x| x as f32), v, true);
                }
            }
        }
    }
    let at = |dx: f32, dy: f32| {
        let s = [src[0] + dx, src[1] + dy, src[2]];
        lightmap::light_point(bsp, s, [s[0], s[1], s[2] - 2048.0], |_| lightmap::STYLE_NORMAL)
    };
    let light = at(0.0, 0.0).unwrap_or([0, 0, 0]);
    let g = |dx, dy| at(dx, dy).map_or(0.0, |c| (c[0] + c[1] + c[2]) as f32 / 768.0);
    let (g0, g1, g2, g3) = (g(-16.0, -16.0), g(16.0, -16.0), g(16.0, 16.0), g(-16.0, 16.0));
    let mut d = [g0 - g1 - g2 + g3, g1 + g0 - g2 - g3, -1.0];
    let len = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
    d = d.map(|x| x / len);
    (light.map(|x| x as f32), d, false)
}

/// Entities drawn with a studio model (.mdl): their own `model` key, or the one the game
/// gives them (hostages).
pub fn model_of(e: &Entity) -> Option<String> {
    let m = e.get("model").map(str::trim).unwrap_or("");
    if m.to_ascii_lowercase().ends_with(".mdl") {
        return Some(m.replace('\\', "/"));
    }
    match e.classname() {
        "hostage_entity" => Some("models/scientist.mdl".into()),
        _ => None,
    }
}

fn look(m: &MapFile) -> Result<Value, Fail> {
    let (texts, from) = config_texts(m);
    let s = read_settings(&texts);
    let bsp = Bsp::open(&m.bsp, false)?;
    let ents = crate::goldsrc::entities::parse(&bsp.entities);
    let sky = sky_name(&ents);
    let sides: Vec<bool> = SIDES.iter().map(|side| sky.as_deref().is_some_and(|n| sky_file(m, n, side).is_some())).collect();
    let sun = sun(&ents);
    let mut lit = Vec::new();
    for (i, e) in ents.iter().enumerate() {
        if lit.len() >= MAX_LIT {
            break;
        }
        let (Some(o), Some(model)) = (e.origin(), model_of(e)) else { continue };
        let (light, dir, from_sky) = model_light(&bsp, o, sun);
        lit.push(json!({"index": i, "model": model, "light": light, "dir": dir, "sky": from_sky}));
    }
    Ok(json!({
        "settings": {
            "gamma": s.gamma, "brightness": s.brightness, "texgamma": s.texgamma, "lightgamma": s.lightgamma,
            "overbright": s.overbright, "texturemode": s.texturemode, "anisotropy": s.anisotropy, "detail": s.detail,
        },
        "settingsFrom": from,
        "sky": sky.map(|n| json!({"name": n, "sides": SIDES.iter().zip(&sides).map(|(k, v)| (k.to_string(), json!(v))).collect::<serde_json::Map<_, _>>()})),
        "sun": sun.map(|(c, d)| json!({"colour": c, "dir": d})),
        "models": lit,
    }))
}

async fn api_look(State(st): State<Shared>, Path(name): Path<String>) -> Response {
    let r = match find_map(&st, &name) {
        Err(f) => Err(f),
        Ok((game, name)) => blocking(move || look(&locate(&game, &name)?)).await,
    };
    json_answer(r)
}

async fn api_sky(State(st): State<Shared>, Path((name, side)): Path<(String, String)>) -> Response {
    let side = side.strip_suffix(".png").unwrap_or(&side).to_ascii_lowercase();
    let r = match find_map(&st, &name) {
        Err(f) => Err(f),
        Ok(_) if !SIDES.contains(&side.as_str()) => Err(Fail::new(StatusCode::BAD_REQUEST, "That isn't a side of the sky.")),
        Ok((game, name)) => {
            blocking(move || {
                let m = locate(&game, &name)?;
                let lite = mapinfo::read_lite(&m.bsp, false)?;
                let sky = sky_name(&lite.entities).ok_or_else(|| Fail::new(StatusCode::NOT_FOUND, "This map's sky has an odd name."))?;
                let path = sky_file(&m, &sky, &side).ok_or_else(|| Fail::new(StatusCode::NOT_FOUND, "That sky picture isn't on this computer."))?;
                let bytes = sky_png(&path)?;
                Ok(Some(Cached { bytes, kind: "image/png", etag: String::new() }))
            })
            .await
        }
    };
    binary_answer(r, None)
}

pub fn routes() -> Router<Shared> {
    Router::new().route("/api/tools/map/:name/look", get(api_look)).route("/api/tools/map/:name/sky/:side", get(api_sky))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::goldsrc::bsp::fixture;
    use crate::goldsrc::entities::parse;

    #[test]
    fn settings_from_config() {
        let s = read_settings(&["gamma \"2.000000\"\nbrightness \"0.5\"\ngl_overbright \"1\"\n// texgamma 9\ngl_texturemode \"GL_LINEAR_MIPMAP_NEAREST\"\nlightgamma 9".into(), "brightness 2".into()]);
        assert_eq!((s.gamma, s.brightness, s.texgamma, s.lightgamma, s.overbright), (2.0, 2.0, 2.0, 3.0, true));
        assert_eq!(s.texturemode, "GL_LINEAR_MIPMAP_NEAREST");
        assert_eq!(read_settings(&[]), Settings::default());
        assert_eq!(read_settings(&["gamma nan\nbrightness \"x\"".into()]), Settings::default());
    }

    #[test]
    fn sky_names() {
        assert!(safe_sky("desert") && safe_sky("3dm_bikini") && safe_sky("cx-1"));
        assert!(!safe_sky("../x") && !safe_sky("a/b") && !safe_sky("") && !safe_sky(".."));
        assert_eq!(sky_name(&parse("{\"classname\" \"worldspawn\"}")).as_deref(), Some("desert"));
        assert_eq!(sky_name(&parse("{\"classname\" \"worldspawn\" \"skyname\" \"..\\\\x\"}")), None);
    }

    #[test]
    fn sun_like_the_game() {
        let e = parse("{\"classname\" \"light_environment\" \"_light\" \"255 255 128 200\" \"angles\" \"0 90 0\" \"pitch\" \"-90\"}");
        let (c, d) = sun(&e).unwrap();
        // 255 * 200/255 = 200 → (200/114)^0.6 * 264 = 369; 128 * 200/255 = 100 → 244
        assert_eq!(c, [369, 369, 244]);
        assert!(d[2] < -0.999, "{d:?}");
        let e = parse("{\"classname\" \"light_environment\" \"_light\" \"114\" \"angle\" \"0\"}");
        let (c, d) = sun(&e).unwrap();
        assert_eq!(c, [264, 264, 264]);
        assert!((d[0] - 1.0).abs() < 1e-5);
        assert!(sun(&parse("{\"classname\" \"light\"}")).is_none());
    }

    #[test]
    fn model_light_in_the_room() {
        let bsp = Bsp::parse(&fixture::build(), false).unwrap();
        // floor light 100 everywhere: 103 after the style, light straight down (no gradient)
        let (l, d, sky) = model_light(&bsp, [0.0, 0.0, -100.0], None);
        assert_eq!((l, sky), ([103.0; 3], false));
        assert!((d[2] + 1.0).abs() < 1e-5);
        // a sun the room's ceiling hides (no sky): still the floor's light
        let (l, _, sky) = model_light(&bsp, [0.0, 0.0, -100.0], Some(([300, 300, 300], [0.0, 0.0, -1.0])));
        assert_eq!((l, sky), ([103.0; 3], false));
        assert_eq!(model_of(&parse("{\"classname\" \"hostage_entity\"}")[0]).as_deref(), Some("models/scientist.mdl"));
        assert_eq!(model_of(&parse("{\"classname\" \"cycler\" \"model\" \"models\\can.mdl\"}")[0]).as_deref(), Some("models/can.mdl"));
        assert_eq!(model_of(&parse("{\"classname\" \"env_sprite\" \"model\" \"sprites/glow.spr\"}")[0]), None);
    }

    /// `LS_TEST_GAME=<Half-Life folder> cargo test real_look -- --ignored --nocapture`: the look
    /// of a few stock maps, and how many maps light faces with more than one style.
    #[test]
    #[ignore]
    fn real_look() {
        let Ok(hl) = std::env::var("LS_TEST_GAME") else { return };
        let hl = PathBuf::from(hl);
        for name in ["de_dust2", "cs_italy", "de_inferno", "de_aztec", "cs_office"] {
            let m = MapFile { name: name.into(), bsp: hl.join(format!("cstrike/maps/{name}.bsp")), game: "cstrike".into(), hl: hl.clone(), czero: false };
            if !m.bsp.is_file() {
                continue;
            }
            let v = look(&m).unwrap();
            println!("{name}: {} {} sun {} models {}", v["settings"], v["sky"], v["sun"], v["models"].as_array().unwrap().len());
            if let Some(first) = v["models"].get(0) {
                println!("   first model {first}");
            }
        }
        let mut multi = Vec::new();
        for e in std::fs::read_dir(hl.join("cstrike/maps")).unwrap().flatten() {
            let p = e.path();
            if p.extension().is_some_and(|x| x == "bsp") {
                if let Ok(bsp) = Bsp::open(&p, false) {
                    let all: Vec<usize> = (0..bsp.faces.len()).collect();
                    let a = lightmap::pack(&bsp, &all);
                    if a.maps > 1 {
                        multi.push(format!("{}:{}", p.file_stem().unwrap().to_string_lossy(), a.maps));
                    }
                }
            }
        }
        println!("maps with light styles: {} {multi:?}", multi.len());
    }
}
