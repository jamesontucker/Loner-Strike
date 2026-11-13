//! The Tools page's server side (the map viewer): `/api/tools/*`, on the app window's
//! listener only. Maps are named, never pathed: names go through `maps::safe_arg` and files
//! are found in the game folders the app already knows.
//!
//! JSON routes answer `{error}` with 200 (400 for a bad name), like the rest of the app.
//! Binary routes (mesh, textures, lightmap) answer `{error}` JSON with an error status
//! instead (404 not there, 422 damaged), so `fetch(...).ok` tells them apart.

pub mod cache;
pub mod mapinfo;
pub mod overlays;
pub mod spray;
pub mod replay;
pub mod navfix;
pub mod health;
pub mod editor;
pub mod doctor;
pub mod callouts;
pub mod assets;
pub mod render;
mod devserve;
pub mod shot;
pub mod overview;
pub mod demos;
pub mod compile;
pub mod plugins;
pub mod community;
pub mod communitylib;
pub mod playercfg;
pub mod people;
pub mod studio;
pub mod creatures;

use crate::goldsrc::{self, bsp::Bsp, entities, mesh, wad};
use crate::server::Shared;
use axum::extract::{Path, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use mapinfo::MapFile;
use serde_json::{json, Value};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

const NO_GAME: &str = "Set up the game on this computer first.";
const BAD_NAME: &str = "That isn't a map name.";
const BAD_TEXTURE: &str = "That isn't a texture name.";
const INTERNAL: &str = "Loner-Strike hit an internal error reading that map. Try again, and copy the diagnostics from Settings if it keeps happening.";

/// Why a request failed: the status for binary routes and the words for the page.
#[derive(Debug)]
struct Fail {
    status: StatusCode,
    msg: String,
}

impl Fail {
    fn new(status: StatusCode, msg: impl Into<String>) -> Fail {
        Fail { status, msg: msg.into() }
    }
    fn no_map() -> Fail {
        Fail::new(StatusCode::NOT_FOUND, mapinfo::NO_MAP)
    }
}

impl From<goldsrc::Error> for Fail {
    fn from(e: goldsrc::Error) -> Fail {
        let status = match e {
            goldsrc::Error::Io(_) => StatusCode::INTERNAL_SERVER_ERROR,
            _ => StatusCode::UNPROCESSABLE_ENTITY,
        };
        Fail::new(status, e.to_string())
    }
}

type Done<T> = Result<T, Fail>;

/// Runs blocking work (file reads, parsing) off the async threads; a panic in a parser
/// becomes the internal-error message, never a dropped connection.
async fn blocking<T: Send + 'static>(f: impl FnOnce() -> Done<T> + Send + 'static) -> Done<T> {
    tokio::task::spawn_blocking(f).await.unwrap_or_else(|_| Err(Fail::new(StatusCode::INTERNAL_SERVER_ERROR, INTERNAL)))
}

fn json_answer(r: Done<Value>) -> Response {
    match r {
        Ok(v) => Json(v).into_response(),
        Err(f) if f.status == StatusCode::BAD_REQUEST => (f.status, Json(json!({"error": f.msg}))).into_response(),
        Err(f) => Json(json!({"error": f.msg})).into_response(),
    }
}

/// A cached file, or `{error}` with its status. Revalidated by ETag on every use.
struct Cached {
    bytes: Vec<u8>,
    kind: &'static str,
    etag: String,
}

fn binary_answer(r: Done<Option<Cached>>, etag_only: Option<String>) -> Response {
    match r {
        Ok(Some(c)) => ([(header::CONTENT_TYPE, c.kind.to_string()), (header::CACHE_CONTROL, "no-cache".into()), (header::ETAG, c.etag)], c.bytes).into_response(),
        // the page already has this version
        Ok(None) => (StatusCode::NOT_MODIFIED, [(header::CACHE_CONTROL, "no-cache".to_string()), (header::ETAG, etag_only.unwrap_or_default())]).into_response(),
        Err(f) => (f.status, Json(json!({"error": f.msg}))).into_response(),
    }
}

fn if_none_match(h: &HeaderMap) -> Option<String> {
    h.get(header::IF_NONE_MATCH).and_then(|v| v.to_str().ok()).map(String::from)
}

/// The player's game and the map, checked.
fn find_map(st: &Shared, name: &str) -> Done<(Arc<crate::game::Game>, String)> {
    if !crate::maps::safe_arg(name) {
        return Err(Fail::new(StatusCode::BAD_REQUEST, BAD_NAME));
    }
    let game = st.local_game().ok_or_else(|| Fail::new(StatusCode::CONFLICT, NO_GAME))?;
    Ok((game, name.to_string()))
}

fn locate(game: &crate::game::Game, name: &str) -> Done<MapFile> {
    mapinfo::locate(game, name).ok_or_else(Fail::no_map)
}

// ---- mesh + lightmap ------------------------------------------------------------------

const MESH_FILES: [&str; 2] = ["mesh.lsmb", "lightmap.png"];

/// The mesh bundle (`want` 0) or the lightmap atlas (1) for a map, built together and cached.
/// None when the page's `If-None-Match` already names this version.
fn mesh_file(m: &MapFile, want: usize, seen: Option<&str>) -> Done<Option<Cached>> {
    let stamp = cache::Stamp::of(&m.bsp).map_err(|_| Fail::no_map())?;
    let dir = cache::map_dir(&cache::root(), &m.name, &stamp);
    let etag = cache::etag(&dir, MESH_FILES[want]);
    if seen == Some(etag.as_str()) && dir.join(MESH_FILES[want]).is_file() {
        return Ok(None);
    }
    let bytes = cache::get_or_build(&dir, &MESH_FILES, want, || -> Done<Vec<Vec<u8>>> {
        let bsp = Bsp::open(&m.bsp, false)?;
        let ents = entities::parse(&bsp.entities);
        let built = mesh::build(&bsp, &ents, &m.name)?;
        let png = goldsrc::png(built.atlas.width, built.atlas.full_height(), &built.atlas.rgb, false)?;
        Ok(vec![built.bundle, png])
    })?;
    let kind = if want == 0 { "application/octet-stream" } else { "image/png" };
    Ok(Some(Cached { bytes, kind, etag }))
}

async fn api_mesh(State(st): State<Shared>, Path(name): Path<String>, h: HeaderMap) -> Response {
    mesh_route(st, name, h, 0).await
}

async fn api_lightmap(State(st): State<Shared>, Path(name): Path<String>, h: HeaderMap) -> Response {
    mesh_route(st, name, h, 1).await
}

async fn mesh_route(st: Shared, name: String, h: HeaderMap, want: usize) -> Response {
    let seen = if_none_match(&h);
    let r = match find_map(&st, &name) {
        Err(f) => Err(f),
        Ok((game, name)) => {
            let seen = seen.clone();
            blocking(move || mesh_file(&locate(&game, &name)?, want, seen.as_deref())).await
        }
    };
    binary_answer(r, seen)
}

// ---- textures ------------------------------------------------------------------------

/// A texture name from the URL ("{grate.png"): up to 16 visible characters, no slashes.
fn texture_name(file: &str) -> Option<String> {
    let n = file.strip_suffix(".png").or_else(|| file.strip_suffix(".PNG")).unwrap_or(file);
    (!n.is_empty() && n.len() <= 16 && n.bytes().all(|c| c.is_ascii_graphic() && c != b'/' && c != b'\\')).then(|| n.to_string())
}

/// What the texture route needs for one map version, kept for a minute: the map's textures
/// (with pixels) and its WADs opened (a map's 200 textures arrive as 200 requests).
struct TexSource {
    bsp: std::path::PathBuf,
    stamp: cache::Stamp,
    lite: mapinfo::Lite,
    wads: wad::WadSet,
    wads_stamp: String,
    at: Instant,
}

fn tex_source(m: &MapFile) -> Done<Arc<TexSource>> {
    static MEMO: OnceLock<Mutex<Vec<Arc<TexSource>>>> = OnceLock::new();
    let memo = MEMO.get_or_init(Default::default);
    let stamp = cache::Stamp::of(&m.bsp).map_err(|_| Fail::no_map())?;
    {
        let mut list = memo.lock().unwrap_or_else(|e| e.into_inner());
        list.retain(|s| s.at.elapsed() < Duration::from_secs(60));
        if let Some(s) = list.iter().find(|s| s.bsp == m.bsp && s.stamp == stamp) {
            return Ok(s.clone());
        }
    }
    let lite = mapinfo::read_lite(&m.bsp, true)?;
    let wads = m.wads(&crate::mapcopy::Finder::default(), &entities::wad_list(&lite.entities));
    let wads_stamp = mapinfo::wads_stamp(&wads);
    let s = Arc::new(TexSource { bsp: m.bsp.clone(), stamp, lite, wads, wads_stamp, at: Instant::now() });
    let mut list = memo.lock().unwrap_or_else(|e| e.into_inner());
    list.push(s.clone());
    if list.len() > 3 {
        list.remove(0);
    }
    Ok(s)
}

/// The texture as RGBA from the map, else the first WAD that has it; None if neither does.
fn decode_texture(src: &TexSource, name: &str) -> Done<Option<wad::Rgba>> {
    let own = src.lite.textures.iter().find(|t| t.name.eq_ignore_ascii_case(name));
    if let Some(rec) = own.and_then(|t| t.embedded.as_ref()) {
        return Ok(Some(wad::decode_miptex(rec, name)?));
    }
    match src.wads.read(name)? {
        Some(rec) => Ok(Some(wad::decode_miptex(&rec, name)?)),
        None => Ok(None),
    }
}

fn texture_file(m: &MapFile, name: &str, seen: Option<&str>) -> Done<Option<Cached>> {
    let src = tex_source(m)?;
    let dir = cache::map_dir(&cache::root(), &m.name, &src.stamp);
    let file = format!("tex-{}-{}", src.wads_stamp, cache::safe_file(name, "png"));
    let etag = cache::etag(&dir, &file);
    if seen == Some(etag.as_str()) && dir.join(&file).is_file() {
        return Ok(None);
    }
    let bytes = cache::get_or_build(&dir, &[file.as_str()], 0, || -> Done<Vec<Vec<u8>>> {
        let t = decode_texture(&src, name)?.ok_or_else(|| {
            let known = src.lite.textures.iter().any(|t| t.name.eq_ignore_ascii_case(name));
            Fail::new(StatusCode::NOT_FOUND, if known { "That texture's file isn't on this computer." } else { "This map doesn't use that texture." })
        })?;
        Ok(vec![goldsrc::png(t.width, t.height, &t.pixels, true)?])
    })?;
    Ok(Some(Cached { bytes, kind: "image/png", etag }))
}

async fn api_texture(State(st): State<Shared>, Path((name, file)): Path<(String, String)>, h: HeaderMap) -> Response {
    let seen = if_none_match(&h);
    let r = match (find_map(&st, &name), texture_name(&file)) {
        (Err(f), _) => Err(f),
        (_, None) => Err(Fail::new(StatusCode::BAD_REQUEST, BAD_TEXTURE)),
        (Ok((game, name)), Some(tex)) => {
            let seen = seen.clone();
            blocking(move || texture_file(&locate(&game, &name)?, &tex, seen.as_deref())).await
        }
    };
    binary_answer(r, seen)
}

// ---- JSON: maps, info, entities -------------------------------------------------------

async fn api_maps(State(st): State<Shared>) -> Response {
    let Some(game) = st.local_game() else { return json_answer(Err(Fail::new(StatusCode::CONFLICT, NO_GAME))) };
    json_answer(blocking(move || Ok(json!({"maps": mapinfo::all_facts(&game)}))).await)
}

async fn api_info(State(st): State<Shared>, Path(name): Path<String>) -> Response {
    let r = match find_map(&st, &name) {
        Err(f) => Err(f),
        Ok((game, name)) => {
            blocking(move || {
                let m = locate(&game, &name)?;
                let lite = mapinfo::read_lite(&m.bsp, false)?;
                let wads = m.wads(&crate::mapcopy::Finder::default(), &entities::wad_list(&lite.entities));
                Ok(mapinfo::info(&m, &lite, &wads))
            })
            .await
        }
    };
    json_answer(r)
}

async fn api_entities(State(st): State<Shared>, Path(name): Path<String>) -> Response {
    let r = match find_map(&st, &name) {
        Err(f) => Err(f),
        Ok((game, name)) => {
            blocking(move || {
                let m = locate(&game, &name)?;
                Ok(mapinfo::entities_json(&mapinfo::read_lite(&m.bsp, false)?))
            })
            .await
        }
    };
    json_answer(r)
}

/// The app window's listener only (merged with the other computer-only routes).
pub fn local_routes() -> Router<Shared> {
    Router::new()
        .route("/api/tools/maps", get(api_maps))
        .route("/api/tools/map/:name/info", get(api_info))
        .route("/api/tools/map/:name/mesh", get(api_mesh))
        .route("/api/tools/map/:name/lightmap.png", get(api_lightmap))
        .route("/api/tools/map/:name/tex/:file", get(api_texture))
        .route("/api/tools/map/:name/entities", get(api_entities))
        .merge(overlays::routes())
        .merge(spray::routes())
        .merge(navfix::routes())
        .merge(health::routes())
        .merge(editor::routes())
        .merge(doctor::routes())
        .merge(callouts::routes())
        .merge(assets::routes())
        .merge(render::routes())
        .merge(shot::routes())
        .merge(overview::routes())
        .merge(demos::routes())
        .merge(compile::routes())
        .merge(plugins::routes())
        .merge(community::routes())
        .merge(communitylib::routes())
        .merge(playercfg::routes())
        .merge(people::routes())
        .merge(studio::routes())
        .merge(creatures::routes())
}

/// Builds everything the viewer needs for one map file, uncached (tests and the real-map
/// check): (triangles, lightmap atlas size, textures found, textures missing).
#[cfg(test)]
pub fn check_map(m: &MapFile) -> Result<(usize, (u32, u32), usize, Vec<String>), String> {
    let bsp = Bsp::open(&m.bsp, false).map_err(|e| e.to_string())?;
    let ents = entities::parse(&bsp.entities);
    let built = mesh::build(&bsp, &ents, &m.name).map_err(|e| e.to_string())?;
    let lite = mapinfo::read_lite(&m.bsp, true).map_err(|e| e.to_string())?;
    let wads = m.wads(&crate::mapcopy::Finder::default(), &entities::wad_list(&lite.entities));
    let wads_stamp = mapinfo::wads_stamp(&wads);
    let stamp = cache::Stamp::of(&m.bsp).map_err(|e| e.to_string())?;
    let src = TexSource { bsp: m.bsp.clone(), stamp, lite, wads, wads_stamp, at: Instant::now() };
    let (mut found, mut missing) = (0, Vec::new());
    for (name, ..) in mapinfo::texture_sources(&src.lite, &src.wads) {
        match decode_texture(&src, &name) {
            Ok(Some(t)) => {
                assert_eq!(t.pixels.len() as u32, t.width * t.height * 4);
                found += 1;
            }
            Ok(None) => missing.push(name),
            Err(e) => missing.push(format!("{name} ({})", e.msg)),
        }
    }
    // LS_TEST_DUMP=<folder>: also save each map's lightmap atlas there, to look at
    if let Ok(dir) = std::env::var("LS_TEST_DUMP") {
        let png = goldsrc::png(built.atlas.width, built.atlas.full_height(), &built.atlas.rgb, false).map_err(|e| e.to_string())?;
        std::fs::write(std::path::Path::new(&dir).join(format!("{}-lightmap.png", m.name)), png).ok();
    }
    Ok((built.triangles, (built.atlas.width, built.atlas.height), found, missing))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::goldsrc::bsp::fixture;
    use crate::server::AppState;
    use axum::body::Body;
    use axum::http::Request;
    use std::sync::RwLock;
    use tower::ServiceExt;

    /// A Half-Life folder with the cube room in cstrike/maps and test.wad in valve/.
    fn game(tag: &str) -> (std::path::PathBuf, Arc<crate::game::Game>) {
        let root = std::env::temp_dir().join(format!("ls-tools-{tag}-{}", std::process::id()));
        let hl = root.join("Half-Life");
        std::fs::create_dir_all(hl.join("cstrike/maps")).unwrap();
        std::fs::create_dir_all(hl.join("valve")).unwrap();
        std::fs::write(hl.join("cstrike/maps/Room.bsp"), fixture::build()).unwrap();
        std::fs::write(hl.join("valve/test.wad"), wad::build_wad(&[("{grate", fixture::miptex("{grate", true))])).unwrap();
        let g = crate::game::Game::with_app_dir(&hl, "cstrike", &root.join("app"));
        (root, Arc::new(g))
    }

    fn state(g: Option<Arc<crate::game::Game>>) -> Shared {
        Arc::new(AppState {
            current: RwLock::new(g),
            hosts: RwLock::new(vec!["127.0.0.1:8027".into()]),
            lan_hosts: RwLock::new(vec!["192.168.1.5:8028".into()]),
            token: RwLock::new("testtoken0123456789".into()),
            app: OnceLock::new(),
            phone: Mutex::new(crate::app_api::Phone::default()),
        })
    }

    async fn call(st: &Shared, uri: &str, etag: Option<&str>) -> (StatusCode, HeaderMap, Vec<u8>) {
        let mut req = Request::builder().uri(uri).header("host", "127.0.0.1:8027");
        if let Some(e) = etag {
            req = req.header(header::IF_NONE_MATCH, e);
        }
        let res = local_routes().with_state(st.clone()).oneshot(req.body(Body::empty()).unwrap()).await.unwrap();
        let (status, headers) = (res.status(), res.headers().clone());
        (status, headers, axum::body::to_bytes(res.into_body(), 64 << 20).await.unwrap().to_vec())
    }

    fn json_of(b: &[u8]) -> Value {
        serde_json::from_slice(b).unwrap()
    }

    #[tokio::test]
    async fn routes_over_the_cube_room() {
        let (root, g) = game("routes");
        let st = state(Some(g));

        let (s, _, b) = call(&st, "/api/tools/maps", None).await;
        let v = json_of(&b);
        assert_eq!(s, StatusCode::OK);
        let m = &v["maps"][0];
        assert_eq!(m["name"], "Room");
        assert_eq!(m["missingWads"], json!(["missing.wad"]));
        assert!(m["nav"].is_null());

        let (_, _, b) = call(&st, "/api/tools/map/room/info", None).await;
        let v = json_of(&b);
        assert_eq!(v["name"], "Room", "{v}");
        assert_eq!(v["models"], 2);
        assert_eq!(v["textures"], json!([{"name": "brick", "width": 16, "height": 16, "from": "embedded"}, {"name": "{grate", "width": 16, "height": 16, "from": "test.wad"}]));
        assert_eq!(v["missingWads"], json!(["missing.wad"]));
        assert_eq!(v["spawns"]["ct"][0]["yaw"], 90.0);
        assert_eq!(v["spawns"]["t"][0]["origin"], json!([64.0, 0.0, -92.0]));
        assert_eq!(v["message"], "Test Room");
        assert_eq!(v["classes"]["func_wall"], 1);

        let (_, _, b) = call(&st, "/api/tools/map/room/entities", None).await;
        let v = json_of(&b);
        assert_eq!(v.as_array().unwrap().len(), 4);
        assert_eq!(v[1]["model"], "*1");

        // the mesh: built, then answered from the cache, then "not modified"
        let (s, h, mesh) = call(&st, "/api/tools/map/room/mesh", None).await;
        assert_eq!(s, StatusCode::OK);
        assert_eq!(&mesh[0..4], b"LSMB");
        assert_eq!(h[header::CACHE_CONTROL], "no-cache");
        let tag = h[header::ETAG].to_str().unwrap().to_string();
        let (s, _, again) = call(&st, "/api/tools/map/room/mesh", None).await;
        assert_eq!((s, again == mesh), (StatusCode::OK, true));
        let (s, _, empty) = call(&st, "/api/tools/map/room/mesh", Some(&tag)).await;
        assert_eq!((s, empty.len()), (StatusCode::NOT_MODIFIED, 0));
        let (header_json, _) = mesh::read_bundle(&mesh);

        let (s, h, png) = call(&st, "/api/tools/map/room/lightmap.png", None).await;
        assert_eq!((s, &h[header::CONTENT_TYPE]), (StatusCode::OK, &"image/png".parse::<axum::http::HeaderValue>().unwrap()));
        let img = image::load_from_memory(&png).unwrap();
        assert_eq!(img.width() as u64, header_json["lightmap"]["width"].as_u64().unwrap());

        // textures: embedded, from the WAD (with `{` see-through), unknown, bad name
        let (s, _, png) = call(&st, "/api/tools/map/room/tex/BRICK.png", None).await;
        assert_eq!(s, StatusCode::OK);
        assert_eq!(image::load_from_memory(&png).unwrap().width(), 16);
        let (s, _, png) = call(&st, "/api/tools/map/room/tex/%7Bgrate.png", None).await;
        assert_eq!(s, StatusCode::OK);
        let img = image::load_from_memory(&png).unwrap().to_rgba8();
        assert_eq!(img.get_pixel(1, 0).0, [1, 1, 1, 255]);
        let (s, _, b) = call(&st, "/api/tools/map/room/tex/nothere.png", None).await;
        assert_eq!(s, StatusCode::NOT_FOUND);
        assert_eq!(json_of(&b)["error"], "This map doesn't use that texture.");
        let (s, _, _) = call(&st, "/api/tools/map/room/tex/a%2Fb.png", None).await;
        assert_eq!(s, StatusCode::BAD_REQUEST);

        // bad names, missing maps
        let (s, _, b) = call(&st, "/api/tools/map/..%2Fx/info", None).await;
        assert_eq!((s, json_of(&b)["error"].as_str()), (StatusCode::BAD_REQUEST, Some(BAD_NAME)));
        let (s, _, b) = call(&st, "/api/tools/map/de_nothere/info", None).await;
        assert_eq!((s, json_of(&b)["error"].as_str()), (StatusCode::OK, Some(mapinfo::NO_MAP)));
        let (s, _, b) = call(&st, "/api/tools/map/de_nothere/mesh", None).await;
        assert_eq!((s, json_of(&b)["error"].as_str()), (StatusCode::NOT_FOUND, Some(mapinfo::NO_MAP)));
        std::fs::remove_dir_all(&root).ok();
        std::fs::remove_dir_all(cache::root()).ok();
    }

    #[tokio::test]
    async fn damaged_maps_and_no_game() {
        let (root, g) = game("damaged");
        std::fs::write(g.hl.join("cstrike/maps/broken.bsp"), &fixture::build()[..300]).unwrap();
        let st = state(Some(g));
        let (s, _, b) = call(&st, "/api/tools/map/broken/mesh", None).await;
        assert_eq!(s, StatusCode::UNPROCESSABLE_ENTITY);
        assert!(json_of(&b)["error"].as_str().unwrap().contains("damaged"));
        let (s, _, b) = call(&st, "/api/tools/map/broken/info", None).await;
        assert_eq!(s, StatusCode::OK);
        assert!(json_of(&b)["error"].as_str().unwrap().contains("damaged"));
        let none = state(None);
        let (_, _, b) = call(&none, "/api/tools/maps", None).await;
        assert_eq!(json_of(&b)["error"], NO_GAME);
        std::fs::remove_dir_all(&root).ok();
    }

    #[tokio::test]
    async fn not_on_the_phone_listener() {
        let st = state(None);
        let req = Request::builder().uri("/api/tools/maps?t=testtoken0123456789").header("host", "192.168.1.5:8028").body(Body::empty()).unwrap();
        let res = crate::server::router(st, crate::server::Listener::Lan).oneshot(req).await.unwrap();
        let b = axum::body::to_bytes(res.into_body(), 1 << 20).await.unwrap();
        assert!(!String::from_utf8_lossy(&b).contains("\"maps\""), "tools answered on the LAN");
    }

    /// Opt-in: LS_TEST_GAME=<Half-Life folder> runs every cstrike (and czero) map through
    /// the mesh, lightmap and texture code. `cargo test real_maps -- --nocapture` prints a line
    /// per map.
    #[test]
    fn real_maps() {
        let Ok(hl) = std::env::var("LS_TEST_GAME") else { return };
        let hl = std::path::PathBuf::from(hl);
        let mut n = 0;
        for game in ["cstrike", "czero"] {
            let mut list: Vec<_> = std::fs::read_dir(hl.join(game).join("maps")).into_iter().flatten().flatten().map(|e| e.path()).collect();
            list.sort();
            for p in list.into_iter().filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("bsp"))) {
                let name = p.file_stem().unwrap().to_string_lossy().to_string();
                let m = MapFile { name: name.clone(), bsp: p.clone(), game: game.into(), hl: hl.clone(), czero: game == "czero" };
                let t0 = Instant::now();
                match check_map(&m) {
                    Ok((tris, atlas, found, missing)) => {
                        assert!(tris > 0, "{name}: no triangles");
                        println!("{game}/{name}: {tris} triangles, lightmap {}x{}, {found} textures, {} missing {:?} ({} ms)", atlas.0, atlas.1, missing.len(), missing.iter().take(3).collect::<Vec<_>>(), t0.elapsed().as_millis());
                    }
                    // a broken file must be an error with words, not a panic
                    Err(e) => println!("{game}/{name}: ERROR {e}"),
                }
                n += 1;
            }
        }
        println!("{n} maps checked");
    }
}
