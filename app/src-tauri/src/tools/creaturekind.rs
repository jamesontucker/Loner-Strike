//! A shared creature on its own (Community's "creature" kind, CP-kind): one of your custom
//! types packed with its custom bases, models and sounds under your name (`author.type`), a
//! preview picture, and the usual manifest and readme. Get installs it as a managed block in
//! creatures.ini (creaturepack::install_pack: the same path a map's or mode's creatures take);
//! an update replaces the block in one write, Go back and a failed update put the old one back
//! (communitylib keeps the block with the rest of the version). Remix copies an installed one
//! into your own part of creatures.ini under your name, crediting the original.
//!
//! Package (the list service's layout, services/serverlist formats.py):
//!   creatures/creatures.ini, models/….mdl, sound/….wav, creatures/preview.png|jpg,
//!   loner-strike.json, README.txt

use super::community::{decode_shot, md5_hex, package_refusal, Item, Kind, Package, PublishReq, Shot, SCHEMA, TAG_TITLE, TAG_VALUE};
use super::communitylib::{Credit, Installed};
use super::creaturepack as cp;
use super::{blocking, json_answer, Fail};
use crate::server::Shared;
use axum::extract::{Path as UrlPath, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};

/// A creature download's limit (a few models and sounds).
pub const MAX_ZIP: u64 = 64 << 20;
const PREVIEW: &str = "creatures/preview";

fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn one_line(s: &str, max: usize) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(max).collect()
}

/// The key a creature remix's credit is kept under (communitylib's remixes.json).
fn credit_key(name: &str) -> String {
    format!("creature:{name}")
}

/// Your custom type `name` in `ini`'s own part (not a built-in, not one installed from Community).
fn own_type(ini: &str, name: &str) -> Result<(), String> {
    let n = name.trim().to_ascii_lowercase();
    if cp::is_builtin(&n) {
        return Err(format!("{n} is one of the game's own types: everyone has it already."));
    }
    if cp::is_shared_name(&n) {
        return Err(format!("{n} came from Community. Remix it to make your own copy, then share that."));
    }
    let (user, _) = cp::split_blocks(ini);
    if !user.lines().any(|l| l.trim().strip_prefix('[').and_then(|r| r.split(']').next()).is_some_and(|s| s.trim().eq_ignore_ascii_case(&n))) {
        return Err(format!("There's no type called {n} in your creatures. Save it in Creature Studio first."));
    }
    Ok(())
}

fn ini_of(hl: &Path, game: &str) -> String {
    crate::sys::read_latin1(&hl.join(game).join(super::creatures::FILE_REL)).unwrap_or_default()
}

/// Is `name` ready to share? What goes with it, what stops it, and the remix credit.
pub fn readiness(hl: &Path, game: &str, lib: &Path, name: &str) -> Result<Value, String> {
    let ini = ini_of(hl, game);
    own_type(&ini, name)?;
    let n = name.trim().to_ascii_lowercase();
    let l = cp::listing(hl, game, std::slice::from_ref(&n));
    let blockers: Vec<Value> = l["problem"].as_str().map(|p| json!(p)).into_iter().collect();
    let credit = super::communitylib::credits(lib).get(&credit_key(&n)).cloned();
    // (the publisher's GameBanana member id when this computer knows it: their creatures' namespace)
    Ok(json!({"name": n, "includes": l["includes"], "blockers": blockers, "remixOf": credit, "member": super::community::known_member()}))
}

#[derive(Deserialize, Default, Clone)]
#[serde(default)]
pub struct CreaturePublishReq {
    #[serde(flatten)]
    pub publish: PublishReq,
    /// the preview picture (PNG or JPEG, base64): the Studio's 3D view
    pub preview: Shot,
}

/// Builds the upload package for your type `name` in `out_dir`.
pub fn build_package(hl: &Path, game: &str, lib: &Path, name: &str, req: &CreaturePublishReq, out_dir: &Path) -> Result<Package, String> {
    let ini = ini_of(hl, game);
    own_type(&ini, name)?;
    let n = name.trim().to_ascii_lowercase();
    let title = one_line(&req.publish.title, 80);
    let author = one_line(&req.publish.author, 40);
    if title.is_empty() || author.is_empty() {
        return Err("Give it a name and say who made it (your GameBanana name).".into());
    }
    // named under your GameBanana member id ("gb<id>.<type>"): nobody else can publish under it
    let member = if req.publish.member > 0 { req.publish.member } else { super::community::known_member() };
    let ns = cp::namespace_of(member).ok_or("Add your GameBanana profile link: your creatures are named for your GameBanana account.")?;
    let pack = cp::pack_types(hl, game, std::slice::from_ref(&n), &ns)?;
    if pack.is_empty() {
        return Err(format!("{n} couldn't be packed."));
    }
    let shared = pack.renames.get(&n).cloned().unwrap_or_else(|| pack.types.last().map(|t| t.name.clone()).unwrap_or_default());
    let (pic, ext) = if req.preview.data.is_empty() { return Err("Add a preview picture (the 3D view).".into()) } else { decode_shot(&req.preview)? };
    let remix_of: Option<Credit> = super::communitylib::credits(lib).get(&credit_key(&n)).cloned();
    let item = Item {
        kind: Kind::Creature,
        name: title.clone(),
        author: author.clone(),
        summary: one_line(&req.publish.summary, 400),
        creatures: pack.item_creatures(),
        remix_of: remix_of.clone(),
        added: now(),
        updated: now(),
        ..Default::default()
    };
    let mut readme = format!(
        "{title}\r\nby {author}\r\n\r\n{}\r\n\r\nA creature for Counter-Strike 1.6 and Condition Zero with Loner-Strike's bots (lonerstrike.com).\r\nIn the app: Tools > Community, Get. Its type is {shared}: modes and maps spawn it by that name.\r\n",
        item.summary
    );
    if let Some(r) = &remix_of {
        readme.push_str(&format!("\r\nA remix of {} by {} (https://gamebanana.com/mods/{}).\r\n", r.name, r.author, r.gb));
    }
    readme.push_str("\r\nBy hand: add creatures/creatures.ini to cstrike/addons/amxmodx/configs/gamemodes/creatures.ini and copy models/ and sound/ into cstrike.\r\n");
    let manifest = json!({"schema": SCHEMA, "app": env!("CARGO_PKG_VERSION"), "item": item});
    let mut entries: Vec<(String, Vec<u8>)> = vec![("README.txt".into(), readme.into_bytes()), ("loner-strike.json".into(), serde_json::to_vec_pretty(&manifest).unwrap_or_default()), (format!("{PREVIEW}.{ext}"), pic.clone())];
    entries.extend(pack.entries(MAX_ZIP)?);
    let contents: Vec<String> = entries.iter().map(|e| e.0.clone()).collect();
    if let Some(why) = package_refusal(&item, &contents) {
        return Err(why);
    }
    std::fs::create_dir_all(out_dir).map_err(|e| format!("Couldn't make the packages folder ({e})."))?;
    let stem = format!("{}-creature-loner-strike", shared.replace('.', "_"));
    let mut buf = std::io::Cursor::new(Vec::new());
    {
        let mut z = zip::ZipWriter::new(&mut buf);
        let opts = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        for (path, body) in &entries {
            z.start_file(path.as_str(), opts).map_err(|e| e.to_string())?;
            z.write_all(body).map_err(|e| e.to_string())?;
        }
        z.finish().map_err(|e| e.to_string())?;
    }
    let bytes = buf.into_inner();
    crate::sys::write_atomic(&out_dir.join(format!("{stem}.zip")), &bytes).map_err(|e| format!("Couldn't save the package ({e})."))?;
    // the preview beside it, for GameBanana's picture upload
    let shot_dir = out_dir.join(format!("{stem}-screenshots"));
    let _ = std::fs::remove_dir_all(&shot_dir);
    std::fs::create_dir_all(&shot_dir).map_err(|e| format!("Couldn't save the preview ({e})."))?;
    let shot = format!("{stem}-1.{ext}");
    std::fs::write(shot_dir.join(&shot), &pic).map_err(|e| format!("Couldn't save the preview ({e})."))?;
    let pkg = Package {
        file: format!("{stem}.zip"),
        size: bytes.len() as u64,
        md5: md5_hex(&bytes),
        sha256: Sha256::digest(&bytes).iter().map(|b| format!("{b:02x}")).collect(),
        contents,
        screenshots: vec![shot],
        item,
        warnings: vec![],
    };
    let _ = std::fs::write(out_dir.join(format!("{stem}.json")), serde_json::to_vec_pretty(&pkg).unwrap_or_default());
    Ok(pkg)
}

/// What to type into GameBanana's submit form for a creature.
pub fn gamebanana_form(p: &Package) -> Value {
    let it = &p.item;
    let names: Vec<&str> = it.creatures.iter().map(|c| c.name.as_str()).collect();
    let mut text = format!(
        "{}\n\nA creature for Loner-Strike's bots (lonerstrike.com): open Tools > Community in the app and click Get. Its type is {}; modes and maps spawn it by that name. Nothing in the download runs code: it's a creatures.ini section with its model and sounds.\n",
        it.summary,
        names.last().copied().unwrap_or("")
    );
    if let Some(r) = &it.remix_of {
        text.push_str(&format!("\n- A remix of {} by {} (https://gamebanana.com/mods/{})", r.name, r.author, r.gb));
    }
    json!({
        "submitUrl": "https://gamebanana.com/mods/add", "game": "Counter-Strike 1.6", "category": "Skins > Monsters",
        "title": it.name, "text": text, "tag": {"title": TAG_TITLE, "value": TAG_VALUE}, "file": p.file, "md5": p.md5,
    })
}

/// Installs a downloaded creature (`src`: the unpacked package) into `game_dir` as the item's
/// block, its types checked against `namespace` (community::install_namespace: the list's
/// verified uploader, never the package), and notes it in the
/// library. Code files and anything but creature files are refused.
pub fn install(game_dir: &Path, src: &Path, namespace: &str, mut note: Installed) -> Result<Value, String> {
    let names: Vec<String> = crate::archive::walk(src).iter().filter_map(|p| p.strip_prefix(src).ok().map(|r| r.to_string_lossy().replace('\\', "/"))).collect();
    if let Some(why) = package_refusal(&Item { kind: Kind::Creature, ..Default::default() }, &names) {
        return Err(why);
    }
    let (_, text) = cp::find_pack(src).ok_or("This download has no creature in it (creatures/creatures.ini).")?;
    let done = cp::install_pack(game_dir, &note.key(), &text, Some(src), namespace)?;
    note.installed = now();
    note.pending = 0;
    super::communitylib::save(&super::communitylib::lib_dir(), &note)?;
    let mut msg = format!("Added {}. Modes and maps can spawn it from the next map.", done.types.join(", "));
    if !done.kept.is_empty() {
        msg.push_str(&format!(" Kept your own {} (a different file of that name).", done.kept.join(", ")));
    }
    Ok(json!({"ok": true, "kind": "creature", "types": done.types, "kept": done.kept, "message": msg}))
}

/// A free name for your copy of `shared`: its type part, then `_2`, `_3`… (Studio name rules).
pub fn remix_name(shared: &str, taken: &dyn Fn(&str) -> bool) -> String {
    let base: String = shared.rsplit('.').next().unwrap_or(shared).chars().filter(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == '_').take(26).collect();
    let base = if base.is_empty() || base.bytes().all(|c| c.is_ascii_digit()) { format!("my_{base}") } else { base };
    let mut n = 1;
    loop {
        let c = if n == 1 { base.clone() } else { format!("{base}_{n}") };
        if !taken(&c) && !cp::is_builtin(&c) {
            return c;
        }
        n += 1;
    }
}

/// Copies the installed shared type `shared` into your creatures as `new_name` (or a free name),
/// crediting the package it came from. Returns your copy's name.
pub fn remix(game_dir: &Path, lib: &Path, shared: &str, new_name: Option<&str>) -> Result<String, String> {
    let shared = shared.trim().to_ascii_lowercase();
    let installed = cp::installed_types(game_dir);
    let from = installed.get(&shared).and_then(|ids| ids.first()).cloned().ok_or_else(|| format!("{shared} isn't installed."))?;
    let ini = crate::sys::read_latin1(&game_dir.join(super::creatures::FILE_REL)).unwrap_or_default();
    let (user, _) = cp::split_blocks(&ini);
    let have: Vec<String> = super::creatures::resolve(&super::creatures::Doc::parse(&user)).into_iter().map(|r| r.t.name).collect();
    let name = match new_name.map(str::trim).filter(|n| !n.is_empty()) {
        Some(n) => n.to_ascii_lowercase(),
        None => remix_name(&shared, &|c| have.iter().any(|h| h.eq_ignore_ascii_case(c))),
    };
    cp::remix_type(game_dir, &shared, &name)?;
    // the credit (shown when you share your copy)
    let credit = super::communitylib::parse_key(&from)
        .map(|(model, gb)| {
            let it = super::communitylib::load(lib, model, gb);
            Credit { uid: String::new(), model, gb, name: it.as_ref().map(|i| i.name.clone()).unwrap_or_else(|| shared.clone()), author: it.map(|i| i.author).unwrap_or_default() }
        })
        .unwrap_or_else(|| Credit { name: shared.clone(), ..Default::default() });
    save_credit(lib, &credit_key(&name), credit)?;
    Ok(name)
}

/// Adds a remix credit to communitylib's remixes.json (the same file mode remixes use).
fn save_credit(lib: &Path, key: &str, c: Credit) -> Result<(), String> {
    let mut all: BTreeMap<String, Credit> = super::communitylib::credits(lib);
    all.insert(key.to_string(), c);
    std::fs::create_dir_all(lib).map_err(|e| e.to_string())?;
    crate::sys::write_atomic(&lib.join("remixes.json"), &serde_json::to_vec_pretty(&all).unwrap_or_default()).map_err(|e| format!("Couldn't note where it came from ({e})."))
}

// ---- routes --------------------------------------------------------------------------------------

fn forbidden() -> Response {
    (StatusCode::FORBIDDEN, Json(json!({"error": "forbidden"}))).into_response()
}

fn packages_dir() -> PathBuf {
    // (community.rs keeps its packages here too)
    crate::localdata::join(&crate::sys::app_dir().join("tools").join("community"), "packages")
}

fn own_game(st: &Shared) -> Result<std::sync::Arc<crate::game::Game>, Fail> {
    st.local_game().ok_or_else(|| Fail::new(StatusCode::OK, "Set up the game on this computer first."))
}

async fn api_ready(State(st): State<Shared>, UrlPath(name): UrlPath<String>) -> Response {
    json_answer(
        blocking(move || {
            let g = own_game(&st)?;
            readiness(&g.hl, &g.name, &super::communitylib::lib_dir(), &name).map_err(|e| Fail::new(StatusCode::OK, e))
        })
        .await,
    )
}

async fn api_publish(State(st): State<Shared>, h: HeaderMap, UrlPath(name): UrlPath<String>, Json(req): Json<CreaturePublishReq>) -> Response {
    if !crate::panel_api::from_panel(&h) {
        return forbidden();
    }
    json_answer(
        blocking(move || {
            let g = own_game(&st)?;
            let pkg = build_package(&g.hl, &g.name, &super::communitylib::lib_dir(), &name, &req, &packages_dir()).map_err(|e| Fail::new(StatusCode::OK, e))?;
            Ok(json!({"package": pkg, "form": super::community::publish_form(&pkg, &req.publish.ai, req.publish.member)}))
        })
        .await,
    )
}

#[derive(Deserialize)]
struct RemixReq {
    name: String,
    #[serde(default)]
    to: String,
}

async fn api_remix(State(st): State<Shared>, h: HeaderMap, Json(r): Json<RemixReq>) -> Response {
    if !crate::panel_api::from_panel(&h) {
        return forbidden();
    }
    json_answer(
        blocking(move || {
            let g = own_game(&st)?;
            let name = remix(&g.dir, &super::communitylib::lib_dir(), &r.name, Some(r.to.as_str())).map_err(|e| Fail::new(StatusCode::OK, e))?;
            Ok(json!({"ok": true, "name": name, "message": format!("Made {name}, your own copy of {}. Share it when you've made it yours.", r.name)}))
        })
        .await,
    )
}

/// The app window's listener only (merged into tools::local_routes).
pub fn routes() -> Router<Shared> {
    Router::new()
        .route("/api/tools/community/publish-creature/:name", get(api_ready).post(api_publish).layer(axum::extract::DefaultBodyLimit::max(16 << 20)))
        .route("/api/tools/community/remix-creature", post(api_remix))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("ls-ckind-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// A tiny PNG, base64.
    fn png_b64() -> String {
        use base64::Engine;
        let img = image::RgbaImage::from_pixel(4, 4, image::Rgba([200, 30, 30, 255]));
        let mut out = std::io::Cursor::new(Vec::new());
        img.write_to(&mut out, image::ImageFormat::Png).unwrap();
        base64::engine::general_purpose::STANDARD.encode(out.into_inner())
    }

    /// A game folder with your type `qa_runner` (base runner, its own model) in creatures.ini.
    fn game(root: &Path) -> (PathBuf, PathBuf) {
        let hl = root.join("hl");
        let g = hl.join("cstrike");
        std::fs::create_dir_all(g.join("models/qa")).unwrap();
        std::fs::write(g.join("models/qa/runner_x.mdl"), b"IDST fake model").unwrap();
        let ini = g.join(super::super::creatures::FILE_REL);
        std::fs::create_dir_all(ini.parent().unwrap()).unwrap();
        std::fs::write(&ini, "; mine\n[qa_runner]\nbase = runner\nhp = 77\nmodel = models/qa/runner_x.mdl\n").unwrap();
        (hl, g)
    }

    #[test]
    fn kind_of_skins_and_models_is_a_creature() {
        use super::super::community::kind_of;
        assert_eq!(kind_of("Mod", "Skins", "Monsters"), Kind::Creature);
        assert_eq!(kind_of("Mod", "Models", ""), Kind::Creature);
        assert_eq!(kind_of("Mod", "Maps", "Deathrun"), Kind::Map);
        assert_eq!(kind_of("Mod", "Sounds", ""), Kind::Other);
        assert_eq!(Kind::parse("creature"), Some(Kind::Creature));
        assert!(Kind::Creature.can_install());
        assert_eq!(serde_json::to_value(Kind::Creature).unwrap(), json!("creature"));
    }

    #[test]
    fn a_creature_package_has_the_list_services_layout() {
        let root = tmp("pkg");
        let (hl, _) = game(&root);
        let lib = root.join("lib");
        // built-ins, unknown and installed names are refused
        assert!(readiness(&hl, "cstrike", &lib, "zombie").unwrap_err().contains("game's own"));
        assert!(readiness(&hl, "cstrike", &lib, "nope").unwrap_err().contains("no type called nope"));
        assert!(readiness(&hl, "cstrike", &lib, "gb12.thing").unwrap_err().contains("Remix it"));
        let r = readiness(&hl, "cstrike", &lib, "qa_runner").unwrap();
        assert_eq!(r["includes"], json!(["qa_runner (with its model)"]));
        let mut req = CreaturePublishReq::default();
        req.publish.title = "QA Runner".into();
        req.publish.author = "Jo Smith".into();
        req.publish.member = 31337;
        req.publish.summary = "Fast and fragile.".into();
        req.publish.member = 0;
        assert!(build_package(&hl, "cstrike", &lib, "qa_runner", &req, &root.join("out")).unwrap_err().contains("profile link"));
        req.publish.member = 31337;
        assert!(build_package(&hl, "cstrike", &lib, "qa_runner", &req, &root.join("out")).unwrap_err().contains("preview"));
        req.preview = Shot { name: "p.png".into(), data: png_b64() };
        let p = build_package(&hl, "cstrike", &lib, "qa_runner", &req, &root.join("out")).unwrap();
        assert_eq!(p.item.kind, Kind::Creature);
        assert_eq!(p.item.creatures[0].name, "gb31337.qa_runner");
        let name = &p.item.creatures[0].name;
        assert!(cp::is_shared_name(name) && name.ends_with(".qa_runner"), "{name}");
        assert!(p.item.creatures[0].model);
        for f in ["README.txt", "loner-strike.json", "creatures/preview.png", "creatures/creatures.ini", "models/gb31337/qa/runner_x.mdl"] {
            assert!(p.contents.iter().any(|c| c == f), "{f}: {:?}", p.contents);
        }
        let z = zip::ZipArchive::new(std::fs::File::open(root.join("out").join(&p.file)).unwrap()).unwrap();
        assert_eq!(z.len(), p.contents.len());
        let form = gamebanana_form(&p);
        assert_eq!(form["category"], "Skins > Monsters");
        assert!(form["text"].as_str().unwrap().contains(name.as_str()));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn get_update_go_back_and_remix_with_blocks() {
        let root = tmp("life");
        let (hl, g) = game(&root);
        let lib = root.join("lib");
        let mut req = CreaturePublishReq::default();
        req.publish.title = "QA Runner".into();
        req.publish.author = "bob".into();
        req.publish.member = 4242;
        req.preview = Shot { name: "p.png".into(), data: png_b64() };
        let p = build_package(&hl, "cstrike", &lib, "qa_runner", &req, &root.join("out")).unwrap();
        let shared = p.item.creatures[0].name.clone();
        // unpack it, as the download would be
        let src = root.join("src");
        let mut z = zip::ZipArchive::new(std::fs::File::open(root.join("out").join(&p.file)).unwrap()).unwrap();
        z.extract(&src).unwrap();
        // another player's game: their own section stays byte for byte
        let other = root.join("other/cstrike");
        let ini = other.join(super::super::creatures::FILE_REL);
        std::fs::create_dir_all(ini.parent().unwrap()).unwrap();
        let theirs = "; theirs\n[my_brute]\nbase = brute\nhp = 900\n";
        std::fs::write(&ini, theirs).unwrap();
        let key = "mod-4242";
        let text = cp::find_pack(&src).unwrap().1;
        // a package naming its types under someone else's namespace is refused
        assert!(cp::install_pack(&other, key, &text, Some(&src), "gb999").is_err());
        assert!(!std::fs::read_to_string(&ini).unwrap().contains(">>>"));
        cp::install_pack(&other, key, &text, Some(&src), "gb4242").unwrap();
        let after = std::fs::read_to_string(&ini).unwrap();
        assert!(after.starts_with(theirs), "{after}");
        assert!(cp::installed_types(&other).contains_key(&shared));
        assert!(other.join("models/gb4242/qa/runner_x.mdl").is_file());
        // an update replaces the block; the kept block (Go back / a failed update) restores it
        let kept = cp::block_text(&other, key).unwrap();
        let v2 = text.replace("hp = 77", "hp = 88");
        cp::install_pack(&other, key, &v2, Some(&src), "gb4242").unwrap();
        assert!(cp::block_text(&other, key).unwrap().contains("hp = 88"));
        cp::restore_block(&other, key, Some(&kept)).unwrap();
        let back = std::fs::read_to_string(&ini).unwrap();
        assert!(back.contains("hp = 77") && !back.contains("hp = 88") && back.starts_with(theirs));
        // remix: your own copy, credited
        let mine = remix(&other, &lib, &shared, None).unwrap();
        assert_eq!(mine, "qa_runner");
        let ini_now = std::fs::read_to_string(&ini).unwrap();
        let (user, _) = cp::split_blocks(&ini_now);
        assert!(user.contains("[qa_runner]") && user.contains(&format!("Remixed from {shared}")), "{user}");
        let c = super::super::communitylib::credits(&lib).get("creature:qa_runner").cloned().unwrap();
        // (no library entry here: no name to credit, and never the member id)
        assert_eq!((c.gb, c.author.as_str()), (4242, ""));
        // a second remix gets a free name
        assert_eq!(remix(&other, &lib, &shared, None).unwrap(), "qa_runner_2");
        // and sharing your copy carries the credit
        let r = readiness(&other.parent().unwrap().to_path_buf(), "cstrike", &lib, "qa_runner").unwrap();
        assert_eq!(r["remixOf"]["gb"], 4242);
        // uninstall: only the block goes; your remixes stay
        cp::uninstall_pack(&other, key).unwrap();
        let gone = std::fs::read_to_string(&ini).unwrap();
        assert!(gone.starts_with(theirs) && gone.contains("[qa_runner]") && !cp::installed_types(&other).contains_key(&shared));
        let _ = std::fs::remove_dir_all(&root);
        let _ = g;
    }

    #[test]
    fn install_namespace_comes_only_from_the_lists_verified_uploader() {
        use super::super::community::install_namespace;
        let listed = Item { kind: Kind::Creature, owner_id: 4242, ..Default::default() };
        assert_eq!(install_namespace(Some(&listed)).unwrap(), "gb4242");
        assert!(install_namespace(Some(&Item { owner_id: 0, ..listed.clone() })).is_err());
        assert!(install_namespace(None).is_err());
    }

    #[test]
    fn remix_names() {
        assert_eq!(remix_name("bob.runner_x", &|_| false), "runner_x");
        assert_eq!(remix_name("bob.runner_x", &|c| c == "runner_x"), "runner_x_2");
        // a shared type named like a built-in gets another name
        assert_eq!(remix_name("bob.zombie", &|_| false), "zombie_2");
        assert_eq!(remix_name("bob.123", &|_| false), "my_123");
    }
}
