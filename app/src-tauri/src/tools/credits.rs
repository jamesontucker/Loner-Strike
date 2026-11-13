//! Asset credits (docs/notes/Assets.md, steps 1 and 2): where each custom file a map ships came
//! from, who to credit, and whether its terms let it be shared.
//!
//! - **At install** (`rights_of`): every GameBanana download keeps a snapshot of the page's terms
//!   (the License tab's checklist, the license text, the Credits module, the "ported" box, the
//!   submitter) beside the SHA-256 of each file it put in the game (`installed_maps.json` and
//!   `content/record.json`: `rights` and `hashes`). The terms you downloaded under are the ones
//!   that count, and pages change.
//! - **At publish** (`resolve` / `for_publish`): each shipped custom file is traced, in order:
//!   stock game file → GameBanana download (by path, then by hash wherever it was copied) →
//!   community creation (its own credits travel with it) → your own file → a credit you typed →
//!   unknown. The result goes into README.txt, rows for GameBanana's Credits module and the
//!   community index entry (`Item.credits`, checked by the list service).
//! - **Verdicts** for sharing a map with the asset inside (lines "Use parts … on GameBanana" and
//!   "… on another site"): ok / ask (a warning; most pages use GameBanana's default "ask") / no
//!   (Publish refuses until you record that you have permission, with a note).
//!
//! Routes (app window only): `GET /api/tools/credits/map/:name`, `POST /api/tools/credits/permission`,
//! `POST /api/tools/credits/manual`.

use crate::server::Shared;
use axum::extract::{Path as UrlPath, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};

/// The License tab's checklist lines, by the start of their text (the noun changes: Mod, Sound, Spray).
pub const LINES: [&str; 8] = ["install", "redistribute", "modify_gb", "modify_other", "parts_gb", "parts_other", "nsfw", "commercial"];
const MAX_TEXT: usize = 1000;
const MAX_CREDITS: usize = 64;

/// Which checklist line a line of text is ("Use parts of this Mod in another Mod and distribute the
/// Mod on GameBanana" → parts_gb). None for a line this app doesn't know (treated as "ask").
pub fn line_key(line: &str) -> Option<&'static str> {
    let l = line.trim().to_ascii_lowercase();
    let gb = l.ends_with("gamebanana");
    Some(if l.starts_with("download and install") {
        "install"
    } else if l.starts_with("redistribute") {
        "redistribute"
    } else if l.starts_with("modify") {
        if gb { "modify_gb" } else { "modify_other" }
    } else if l.starts_with("use parts") {
        if gb { "parts_gb" } else { "parts_other" }
    } else if l.contains("adult") || l.contains("nsfw") {
        "nsfw"
    } else if l.contains("commercial") {
        "commercial"
    } else {
        return None;
    })
}

fn s(v: &Value) -> String {
    v.as_str().unwrap_or("").to_string()
}

fn one_line(t: &str, max: usize) -> String {
    t.chars().map(|c| if c.is_control() { ' ' } else { c }).collect::<String>().split_whitespace().collect::<Vec<_>>().join(" ").chars().take(max).collect()
}

/// The terms snapshot from a GameBanana ProfilePage (Mod, Sound or Spray), in a fixed shape.
pub fn rights_of(page: &Value, model: &str, id: i64) -> Value {
    let mut checklist = serde_json::Map::new();
    for key in LINES {
        checklist.insert(key.into(), json!("ask")); // a line the page doesn't fill reads as "ask"
    }
    let mut others: Vec<Value> = Vec::new();
    for bucket in ["yes", "ask", "only_if_same", "no"] {
        for line in page["_aLicenseChecklist"][bucket].as_array().into_iter().flatten().filter_map(|l| l.as_str()) {
            match line_key(line) {
                Some(k) => {
                    checklist.insert(k.into(), json!(bucket));
                }
                None => others.push(json!({"text": one_line(line, 200), "bucket": bucket})),
            }
        }
    }
    let credits: Vec<Value> = page["_aCredits"]
        .as_array()
        .into_iter()
        .flatten()
        .take(16)
        .map(|g| {
            let authors: Vec<Value> = g["_aAuthors"]
                .as_array()
                .into_iter()
                .flatten()
                .take(24)
                .map(|a| json!({"name": one_line(&s(&a["_sName"]), 60), "role": one_line(&s(&a["_sRole"]), 60), "url": safe_url(&s(&a["_sProfileUrl"]))}))
                .filter(|a| a["name"] != "")
                .collect();
            json!({"group": one_line(&s(&g["_sGroupName"]), 60), "authors": authors})
        })
        .collect();
    let licence = crate::gamebanana::plain(&s(&page["_sLicense"]), MAX_TEXT);
    let url = if model.eq_ignore_ascii_case("spray") { format!("https://gamebanana.com/sprays/{id}") } else if model.eq_ignore_ascii_case("sound") { format!("https://gamebanana.com/sounds/{id}") } else { format!("https://gamebanana.com/mods/{id}") };
    json!({
        "page": url, "model": model, "id": id,
        "name": one_line(&s(&page["_sName"]), 80),
        "submitter": {"id": page["_aSubmitter"]["_idRow"].as_i64().unwrap_or(0), "name": one_line(&s(&page["_aSubmitter"]["_sName"]), 60)},
        "credits": credits, "ported": page["_bIsPorted"] == json!(true), "createdBySubmitter": page["_bCreatedBySubmitter"] == json!(true),
        "checklist": checklist, "otherLines": others, "licence": licence,
        "fetched": crate::sys::now() as i64,
    })
}

fn safe_url(u: &str) -> String {
    if u.starts_with("https://gamebanana.com/") && u.len() < 200 && !u.contains(['"', '<', '>', ' ']) { u.to_string() } else { String::new() }
}

/// Companies GameBanana never allows ported content from ("Definite No", wiki 755).
const DEFINITE_NO: [&str; 4] = ["bohemia", "cry of fear", "battlestate", "america's army"];
const COMPANIES: [&str; 18] = ["valve", "nexon", "tripwire", "activision", "capcom", "blizzard", "sega", "konami", "rockstar", "nintendo", "crytek", "ubisoft", "epic", "id software", "bohemia", "cry of fear", "battlestate", "america's army"];

/// A company a credits list names (a sign of a port, more reliable than the "ported" box).
pub fn ported_from(rights: &Value) -> Option<String> {
    for g in rights["credits"].as_array().into_iter().flatten() {
        for a in g["authors"].as_array().into_iter().flatten() {
            let n = a["name"].as_str().unwrap_or("").to_lowercase();
            if let Some(c) = COMPANIES.iter().find(|c| n.contains(*c)) {
                return Some(c.to_string());
            }
        }
    }
    rights["ported"].as_bool().filter(|p| *p).map(|_| String::new())
}

/// Whether the terms let the file ship inside a map shared on GameBanana and our index:
/// ("ok" | "ask" | "no", a plain sentence). The stricter of the two lines wins; a page on GameBanana's
/// default terms is described as that, not as the author forbidding it.
pub fn verdict(rights: &Value) -> (&'static str, String) {
    let line = |k: &str| rights["checklist"][k].as_str().unwrap_or("ask").to_string();
    let (gb, other) = (line("parts_gb"), line("parts_other"));
    let name = rights["name"].as_str().filter(|n| !n.is_empty()).unwrap_or("This download");
    let by = rights["submitter"]["name"].as_str().filter(|n| !n.is_empty()).map(|n| format!(" by {n}")).unwrap_or_default();
    if let Some(c) = ported_from(rights).filter(|c| DEFINITE_NO.contains(&c.as_str())) {
        return ("no", format!("{name}{by} is ported from {c}'s content, which GameBanana never allows. Leave it out, or record that you have permission."));
    }
    if gb == "no" || other == "no" {
        return ("no", format!("{name}{by} says you may not use parts of it in something you share. Leave it out, or ask them and record that you have permission."));
    }
    if gb == "yes" && other == "yes" {
        return ("ok", format!("{name}{by} lets you use parts of it in something you share, with credit."));
    }
    if gb == "only_if_same" || other == "only_if_same" {
        return ("ask", format!("{name}{by} allows it only if you offer the same rights. Check its terms, or ask them and record that you have permission."));
    }
    ("ask", format!("{name}{by} uses GameBanana's default terms: ask first before using it in a map you share. Ask them, then mark it Permission granted with a note."))
}

// ---- where a file came from ---------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Author {
    pub name: String,
    pub role: String,
    pub url: String,
}

/// One credit line on the community index and in packages: who made which files.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct AssetCredit {
    /// game paths of the files it covers
    pub files: Vec<String>,
    /// the GameBanana (or community) page, "" for a credit someone typed
    pub page: String,
    pub name: String,
    pub authors: Vec<Author>,
    /// "granted" when the mapper recorded asking and getting a yes
    pub permission: String,
    pub note: String,
}

/// A permission the mapper recorded, or a credit they typed (for a file nothing else knows).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Noted {
    pub granted: bool,
    pub note: String,
    pub date: i64,
    /// typed credit: who made it, and where it's from
    pub authors: Vec<Author>,
    pub page: String,
}

/// Where one asset came from: the shape the Publish page and P5's content browser read.
#[derive(Serialize, Clone, Debug, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct Asset {
    pub rel: String,
    pub sha256: String,
    /// stock | gamebanana | community | mine | manual | unknown
    pub source: String,
    pub page: String,
    pub name: String,
    pub authors: Vec<Author>,
    /// the License tab, line → yes | ask | only_if_same | no (empty when not from GameBanana)
    pub checklist: Value,
    pub ported: String,
    /// ok | ask | no | none (no permission needed: stock or yours)
    pub verdict: String,
    pub why: String,
    pub permission: Option<Noted>,
}

/// What resolving looks at, loaded once per check.
#[derive(Default)]
pub struct Ctx {
    /// lower-case game path → (rights snapshot, page name)
    pub by_path: HashMap<String, Value>,
    /// sha256 → rights snapshot
    pub by_sha: HashMap<String, Value>,
    /// sha256 / lower-case path of files that came with a community creation → its credits
    pub community: HashMap<String, (Value, Vec<AssetCredit>)>,
    /// sha256 of your own files (imported models, sprays you made)
    pub mine: std::collections::HashSet<String>,
    /// page or "sha:<hash>" → what the mapper noted
    pub noted: HashMap<String, Noted>,
}

fn noted_path(app: &Path) -> PathBuf {
    app.join("community").join("credits.json")
}

pub fn load_noted(app: &Path) -> HashMap<String, Noted> {
    std::fs::read(noted_path(app)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

pub fn save_noted(app: &Path, all: &HashMap<String, Noted>) -> Result<(), String> {
    let p = noted_path(app);
    std::fs::create_dir_all(p.parent().unwrap_or(app)).map_err(|e| e.to_string())?;
    crate::safety::before("Community", "record a credit or permission", &p);
    crate::sys::write_atomic(&p, &serde_json::to_vec_pretty(all).unwrap_or_default()).map_err(|e| format!("Couldn't save it ({e})."))
}

impl Ctx {
    /// Reads the install records (maps and content), the community library and your own files.
    pub fn load(app: &Path) -> Ctx {
        let mut c = Ctx { noted: load_noted(app), ..Default::default() };
        let read = |p: PathBuf| std::fs::read(p).ok().and_then(|b| serde_json::from_slice::<Value>(&b).ok()).unwrap_or(Value::Null);
        let maps = read(app.join("installed_maps.json"));
        let content = read(app.join("content").join("record.json"));
        for (_k, r) in maps.as_object().into_iter().flatten().chain(content.as_object().into_iter().flatten()) {
            c.add_record(r);
        }
        for f in read(app.join("tools").join("imported_models.json"))["files"].as_array().into_iter().flatten() {
            if let Some(h) = f["sha256"].as_str() {
                c.mine.insert(h.to_lowercase());
            }
        }
        // community creations: files their maps installed (their own credits ride along)
        for it in super::communitylib::load_all(&super::communitylib::lib_dir()) {
            let manifest_credits: Vec<AssetCredit> = std::fs::read(super::communitylib::lib_dir().join(it.key()).join("credits.json")).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
            let who = json!({"page": it.url(), "name": it.name, "submitter": {"name": it.author}});
            let rec = maps[it.gb.to_string()].clone();
            for f in rec["files"].as_array().into_iter().flatten().filter_map(|f| f.as_str()) {
                c.community.insert(f.to_lowercase(), (who.clone(), manifest_credits.clone()));
            }
        }
        c
    }

    /// One install record's files, by path and by hash.
    pub fn add_record(&mut self, r: &Value) {
        let rights = if r["rights"].is_object() {
            r["rights"].clone()
        } else {
            // (installed before rights were kept: what the record knows)
            json!({"page": r["url"], "name": r["name"], "submitter": {"name": r["author"]}, "credits": [], "checklist": {}})
        };
        for f in r["files"].as_array().into_iter().flatten() {
            let p = f.as_str().map(String::from).or_else(|| f["path"].as_str().map(String::from)).unwrap_or_default();
            if !p.is_empty() {
                self.by_path.insert(p.replace('\\', "/").to_lowercase(), rights.clone());
            }
        }
        for (_, h) in r["hashes"].as_object().into_iter().flatten() {
            if let Some(h) = h.as_str() {
                self.by_sha.insert(h.to_lowercase(), rights.clone());
            }
        }
    }
}

fn authors_of(rights: &Value) -> Vec<Author> {
    let mut out: Vec<Author> = Vec::new();
    for g in rights["credits"].as_array().into_iter().flatten() {
        for a in g["authors"].as_array().into_iter().flatten() {
            let role = [s(&g["group"]), s(&a["role"])].into_iter().filter(|x| !x.is_empty()).collect::<Vec<_>>().join(": ");
            out.push(Author { name: s(&a["name"]), role, url: s(&a["url"]) });
        }
    }
    if out.is_empty() {
        let n = s(&rights["submitter"]["name"]);
        if !n.is_empty() {
            out.push(Author { name: n, role: "Uploader".into(), url: String::new() });
        }
    }
    out
}

/// Where one shipped file came from, in the plan's order.
pub fn resolve(rel: &str, sha: &str, stock: bool, ctx: &Ctx) -> Asset {
    let low = rel.replace('\\', "/").to_lowercase();
    let mut a = Asset { rel: rel.to_string(), sha256: sha.to_string(), checklist: json!({}), ..Default::default() };
    let noted = |key: &str| ctx.noted.get(key).cloned();
    if stock {
        a.source = "stock".into();
        a.verdict = "none".into();
        a.why = "Every player has it already; it isn't shipped or credited.".into();
        return a;
    }
    let gb = ctx.by_path.get(&low).or_else(|| ctx.by_sha.get(sha)).filter(|r| r["page"].is_string());
    if let Some(r) = gb {
        a.source = "gamebanana".into();
        a.page = s(&r["page"]);
        a.name = s(&r["name"]);
        a.authors = authors_of(r);
        a.checklist = r["checklist"].clone();
        a.ported = ported_from(r).unwrap_or_default();
        let (v, why) = if r["checklist"].as_object().is_some_and(|c| !c.is_empty()) {
            verdict(r)
        } else {
            ("ask", format!("{} was installed before Loner-Strike kept downloads' terms. Check its page, then mark it Permission granted if it says you may.", if a.name.is_empty() { "This file" } else { &a.name }))
        };
        a.verdict = v.into();
        a.why = why;
        a.permission = noted(&a.page);
        return a;
    }
    if let Some((who, credits)) = ctx.community.get(&low).or_else(|| ctx.community.get(sha)) {
        a.source = "community".into();
        a.page = s(&who["page"]);
        a.name = s(&who["name"]);
        // the creation's own credits for this file, else its creator
        a.authors = credits.iter().find(|c| c.files.iter().any(|f| f.eq_ignore_ascii_case(rel))).map(|c| c.authors.clone()).unwrap_or_else(|| authors_of(who));
        a.verdict = "ok".into();
        a.why = "Shared on Loner-Strike Community; its creator is credited.".into();
        a.permission = noted(&a.page);
        return a;
    }
    if ctx.mine.contains(sha) {
        a.source = "mine".into();
        a.verdict = "none".into();
        a.why = "Your own file.".into();
        return a;
    }
    if let Some(n) = noted(&format!("sha:{sha}")) {
        a.source = "manual".into();
        a.page = n.page.clone();
        a.authors = n.authors.clone();
        a.verdict = "ok".into();
        a.why = "Credited by you.".into();
        a.permission = Some(n);
        return a;
    }
    a.source = "unknown".into();
    a.verdict = "ask".into();
    a.why = "Loner-Strike doesn't know where this file is from. Add a credit for whoever made it (or mark it as yours).".into();
    a
}

pub fn sha256_file(p: &Path) -> String {
    let Ok(mut f) = std::fs::File::open(p) else { return String::new() };
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 1 << 16];
    loop {
        match f.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => h.update(&buf[..n]),
            Err(_) => return String::new(),
        }
    }
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

/// SHA-256 of each installed file (game path → hash), for the install records.
pub fn hashes(files: &[(String, PathBuf)]) -> Value {
    let mut m = serde_json::Map::new();
    for (rel, p) in files {
        let h = sha256_file(p);
        if !h.is_empty() {
            m.insert(rel.clone(), json!(h));
        }
    }
    Value::Object(m)
}

/// Publish's view of a map's shipped custom files: each asset, the credits list (grouped by
/// source), warnings for "ask" and blockers for "no" without a recorded permission.
pub fn for_publish(assets: &[Asset]) -> (Vec<AssetCredit>, Vec<String>, Vec<String>) {
    let mut credits: Vec<AssetCredit> = Vec::new();
    let (mut warnings, mut blockers) = (Vec::new(), Vec::new());
    for a in assets {
        if a.source == "stock" || a.source == "mine" {
            continue;
        }
        let granted = a.permission.as_ref().is_some_and(|p| p.granted);
        match (a.verdict.as_str(), granted) {
            ("no", false) => blockers.push(format!("{} ({})", a.why, a.rel)),
            ("ask", false) => warnings.push(format!("{} ({})", a.why, a.rel)),
            _ => {}
        }
        if a.source == "unknown" {
            continue;
        }
        let key = if a.page.is_empty() { format!("{:?}", a.authors.iter().map(|x| &x.name).collect::<Vec<_>>()) } else { a.page.clone() };
        match credits.iter_mut().find(|c| (if c.page.is_empty() { format!("{:?}", c.authors.iter().map(|x| &x.name).collect::<Vec<_>>()) } else { c.page.clone() }) == key) {
            Some(c) => c.files.push(a.rel.clone()),
            None => credits.push(AssetCredit {
                files: vec![a.rel.clone()],
                page: a.page.clone(),
                name: a.name.clone(),
                authors: a.authors.clone(),
                permission: if granted { "granted".into() } else { String::new() },
                note: a.permission.as_ref().filter(|p| p.granted).map(|p| p.note.clone()).unwrap_or_default(),
            }),
        }
    }
    credits.truncate(MAX_CREDITS);
    (credits, warnings, blockers)
}

/// Credits as the index and packages may carry them: ≤ 64, short strings, plain paths, GameBanana links only.
pub fn clean_credits(list: Vec<AssetCredit>) -> Vec<AssetCredit> {
    list.into_iter()
        .take(MAX_CREDITS)
        .map(|c| AssetCredit {
            files: c.files.into_iter().filter(|f| f.len() <= 128 && !f.contains("..") && !f.starts_with('/') && !f.contains('\\')).take(64).map(|f| one_line(&f, 128)).collect(),
            page: safe_url(&c.page),
            name: one_line(&c.name, 80),
            authors: c.authors.into_iter().take(24).map(|a| Author { name: one_line(&a.name, 60), role: one_line(&a.role, 60), url: safe_url(&a.url) }).filter(|a| !a.name.is_empty()).collect(),
            permission: if c.permission == "granted" { c.permission } else { String::new() },
            note: one_line(&c.note, 300),
        })
        .filter(|c| !c.files.is_empty())
        .collect()
}

/// README.txt's Credits section.
pub fn readme_section(credits: &[AssetCredit]) -> String {
    if credits.is_empty() {
        return String::new();
    }
    let mut t = String::from("\r\nCredits\r\n-------\r\n");
    for c in credits {
        let who = c.authors.iter().map(|a| if a.role.is_empty() { a.name.clone() } else { format!("{} ({})", a.name, a.role) }).collect::<Vec<_>>().join(", ");
        t.push_str(&format!("{}{}: {}\r\n", if c.name.is_empty() { "Files" } else { &c.name }, if c.page.is_empty() { String::new() } else { format!(" <{}>", c.page) }, if who.is_empty() { "unknown author".into() } else { who }));
        t.push_str(&format!("  {}\r\n", c.files.join(", ")));
        if c.permission == "granted" {
            t.push_str(&format!("  Used with permission{}\r\n", if c.note.is_empty() { String::new() } else { format!(" ({})", c.note) }));
        }
    }
    t
}

/// Rows to paste into GameBanana's Credits module (rule R.23 wants credits there).
pub fn gamebanana_rows(credits: &[AssetCredit]) -> Value {
    let rows: Vec<Value> = credits
        .iter()
        .flat_map(|c| {
            c.authors.iter().map(move |a| {
                json!({"group": if c.name.is_empty() { "Assets".to_string() } else { c.name.clone() }, "name": a.name, "role": if a.role.is_empty() { "Assets used".to_string() } else { a.role.clone() }, "url": a.url})
            })
        })
        .collect();
    json!(rows)
}

/// The credits for every asset of a map shipped by Publish (stock files marked as such).
pub fn assets_for(m: &super::mapinfo::MapFile, files: &[String], ctx: &Ctx) -> Vec<Asset> {
    let f = crate::mapcopy::Finder::default();
    let path = super::doctor::game_path(&m.hl, &m.game);
    files
        .iter()
        .map(|rel| {
            let p = path.iter().find_map(|d| f.find(d, rel));
            let sha = p.as_deref().map(sha256_file).unwrap_or_default();
            resolve(rel, &sha, super::doctor::is_stock(&f, &m.hl, rel), ctx)
        })
        .collect()
}

// ---- routes ---------------------------------------------------------------------------------------

fn forbidden() -> Response {
    (StatusCode::FORBIDDEN, Json(json!({"error": "forbidden"}))).into_response()
}

async fn run(f: impl FnOnce() -> Result<Value, String> + Send + 'static) -> Response {
    match tokio::task::spawn_blocking(f).await {
        Ok(Ok(v)) => Json(v).into_response(),
        Ok(Err(e)) => Json(json!({"error": e})).into_response(),
        Err(_) => Json(json!({"error": "Loner-Strike hit an internal error. Try again."})).into_response(),
    }
}

async fn api_map(State(st): State<Shared>, UrlPath(name): UrlPath<String>) -> Response {
    let game = st.local_game();
    run(move || {
        if !crate::maps::safe_arg(&name) {
            return Err("That isn't a map name.".into());
        }
        let g = game.ok_or("Set up the game on this computer first.")?;
        let m = super::mapinfo::locate(&g, &name).ok_or("That map isn't on this computer.")?;
        let r = super::community::readiness(&m, &crate::sys::app_dir().join("tools").join("health"));
        let ctx = Ctx::load(&crate::sys::app_dir());
        let assets = assets_for(&m, &r.files, &ctx);
        let (credits, warnings, blockers) = for_publish(&assets);
        Ok(json!({"assets": assets, "credits": credits, "warnings": warnings, "blockers": blockers}))
    })
    .await
}

#[derive(Deserialize)]
struct PermissionReq {
    /// the page (for a GameBanana or community file), or "sha:<hash>" for a file nothing knows
    key: String,
    #[serde(default)]
    granted: bool,
    #[serde(default)]
    note: String,
    #[serde(default)]
    authors: Vec<Author>,
    #[serde(default)]
    page: String,
}

fn clean_key(k: &str) -> Option<String> {
    let k = k.trim();
    let ok = (k.starts_with("https://gamebanana.com/") && k.len() < 200 && !k.contains(['"', '<', '>', ' ']))
        || (k.len() == 68 && k.starts_with("sha:") && k[4..].bytes().all(|c| c.is_ascii_hexdigit()));
    ok.then(|| k.to_lowercase())
}

/// Records (or clears) "permission granted" with a note, or a typed credit for an unknown file.
pub fn note(app: &Path, key: &str, granted: bool, note: &str, authors: Vec<Author>, page: &str) -> Result<Value, String> {
    let key = clean_key(key).ok_or("That isn't a page or a file to note.")?;
    let note = one_line(note, 300);
    if granted && note.is_empty() {
        return Err("Say how you got permission (who said yes, where, when).".into());
    }
    let authors: Vec<Author> = authors
        .into_iter()
        .take(12)
        .map(|a| Author { name: one_line(&a.name, 60), role: one_line(&a.role, 60), url: safe_url(&a.url) })
        .filter(|a| !a.name.is_empty())
        .collect();
    let mut all = load_noted(app);
    if !granted && authors.is_empty() {
        all.remove(&key);
    } else {
        all.insert(key.clone(), Noted { granted, note, date: crate::sys::now() as i64, authors, page: safe_url(page) });
    }
    save_noted(app, &all)?;
    Ok(json!({"ok": true, "key": key}))
}

async fn api_permission(h: HeaderMap, Json(req): Json<PermissionReq>) -> Response {
    if !crate::panel_api::from_panel(&h) {
        return forbidden();
    }
    run(move || note(&crate::sys::app_dir(), &req.key, req.granted, &req.note, req.authors, &req.page)).await
}

pub fn routes() -> Router<Shared> {
    Router::new()
        .route("/api/tools/credits/map/:name", get(api_map))
        .route("/api/tools/credits/permission", post(api_permission))
        .route("/api/tools/credits/manual", post(api_permission))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page(yes: &[&str], ask: &[&str], no: &[&str]) -> Value {
        json!({
            "_sName": "Civilian Zombies", "_aSubmitter": {"_idRow": 599043, "_sName": "DimeX"},
            "_aLicenseChecklist": {"yes": yes, "ask": ask, "no": no},
            "_sLicense": "<a href=\"x\">This work is licensed under CC BY-NC-ND 4.0</a>",
            "_aCredits": [{"_sGroupName": "Original Authors", "_aAuthors": [{"_sName": "DimeX", "_sProfileUrl": "https://gamebanana.com/members/599043"}]}],
            "_bIsPorted": false, "_bCreatedBySubmitter": true,
        })
    }

    const ALL: [&str; 8] = [
        "Download and install this Mod", "Redistribute this Mod on other sites", "Modify this Mod and distribute the modified Mod on GameBanana",
        "Modify this Mod and distribute the modified Mod on another site", "Use parts of this Mod in another Mod and distribute the Mod on GameBanana",
        "Use parts of this Mod in another Mod and distribute the Mod on another site", "Use this Mod or components of this Mod in adult/NSFW content",
        "Use this Mod or components of this Mod commercially",
    ];

    #[test]
    fn checklist_lines_whatever_the_noun() {
        let keys: Vec<_> = ALL.iter().map(|l| line_key(l).unwrap()).collect();
        assert_eq!(keys, LINES);
        assert_eq!(line_key("Use parts of this Sound in another Sound and distribute the Sound on GameBanana"), Some("parts_gb"));
        assert_eq!(line_key("Something new"), None);
    }

    #[test]
    fn rights_snapshot_and_verdicts() {
        // GameBanana's default: install yes, commercial no, the rest ask
        let r = rights_of(&page(&ALL[..1], &ALL[1..7], &ALL[7..]), "Mod", 227562);
        assert_eq!(r["checklist"]["install"], "yes");
        assert_eq!(r["checklist"]["parts_gb"], "ask");
        assert_eq!(r["checklist"]["commercial"], "no");
        assert_eq!(r["licence"], "This work is licensed under CC BY-NC-ND 4.0", "plain text");
        assert_eq!(r["submitter"]["id"], 599043);
        assert_eq!(r["credits"][0]["authors"][0]["name"], "DimeX");
        let (v, why) = verdict(&r);
        assert_eq!(v, "ask");
        assert!(why.contains("default terms") && why.contains("DimeX"), "{why}");
        let open = rights_of(&page(&ALL[..6], &[], &ALL[6..]), "Mod", 1);
        assert_eq!(verdict(&open).0, "ok");
        let no = rights_of(&page(&ALL[..1], &ALL[1..5], &ALL[5..]), "Mod", 1);
        assert_eq!(verdict(&no).0, "no", "either sharing line no is no");
        let mut ported = open.clone();
        ported["credits"] = json!([{"group": "Original Authors", "authors": [{"name": "Cry of Fear team"}]}]);
        assert_eq!(verdict(&ported).0, "no", "GameBanana's Definite No list");
        assert_eq!(ported_from(&ported).as_deref(), Some("cry of fear"));
    }

    fn ctx_with(r: Value) -> Ctx {
        let mut c = Ctx::default();
        c.add_record(&json!({"url": r["page"], "name": "Civilian Zombies", "files": ["models/player/zombie/zombie.mdl", {"path": "sound/zombie/roar.wav"}], "hashes": {"models/copied.mdl": "aa"}, "rights": r}));
        c
    }

    #[test]
    fn resolving_in_order() {
        let r = rights_of(&page(&ALL[..1], &ALL[1..7], &ALL[7..]), "Mod", 227562);
        let mut ctx = ctx_with(r);
        assert_eq!(resolve("models/v_ak47.mdl", "x", true, &ctx).source, "stock");
        let a = resolve("Models/Player/Zombie/zombie.mdl", "zz", false, &ctx);
        assert_eq!((a.source.as_str(), a.verdict.as_str(), a.page.as_str()), ("gamebanana", "ask", "https://gamebanana.com/mods/227562"));
        assert_eq!(resolve("sound/zombie/roar.wav", "", false, &ctx).source, "gamebanana", "content records' file objects");
        assert_eq!(resolve("models/props/moved_here.mdl", "aa", false, &ctx).source, "gamebanana", "found by hash after a copy");
        ctx.mine.insert("bb".into());
        assert_eq!(resolve("models/mine.mdl", "bb", false, &ctx).verdict, "none");
        ctx.noted.insert("sha:cc".into(), Noted { authors: vec![Author { name: "Pal".into(), ..Default::default() }], ..Default::default() });
        assert_eq!(resolve("models/typed.mdl", "cc", false, &ctx).source, "manual");
        let u = resolve("models/what.mdl", "dd", false, &ctx);
        assert_eq!((u.source.as_str(), u.verdict.as_str()), ("unknown", "ask"));
        // an old install with no terms kept: ask, with a pointer to check the page
        let mut old = Ctx::default();
        old.add_record(&json!({"url": "https://gamebanana.com/mods/5", "name": "Old", "files": ["old.wad"]}));
        let o = resolve("old.wad", "", false, &old);
        assert_eq!((o.source.as_str(), o.verdict.as_str()), ("gamebanana", "ask"));
        assert!(o.why.contains("before Loner-Strike kept"));
    }

    #[test]
    fn publish_credits_warnings_and_refusals() {
        let ask = rights_of(&page(&ALL[..1], &ALL[1..7], &ALL[7..]), "Mod", 227562);
        let no = rights_of(&page(&ALL[..1], &ALL[1..5], &ALL[5..]), "Mod", 99);
        let mut ctx = ctx_with(ask);
        ctx.add_record(&json!({"url": no["page"], "files": ["models/forbidden.mdl"], "rights": no}));
        let assets = vec![
            resolve("models/player/zombie/zombie.mdl", "", false, &ctx),
            resolve("sound/zombie/roar.wav", "", false, &ctx),
            resolve("models/forbidden.mdl", "", false, &ctx),
            resolve("models/v_ak47.mdl", "", true, &ctx),
        ];
        let (credits, warnings, blockers) = for_publish(&assets);
        assert_eq!(credits.len(), 2, "grouped by page");
        assert_eq!(credits[0].files.len(), 2);
        assert_eq!(warnings.len(), 2, "ask: a warning each");
        assert_eq!(blockers.len(), 1, "no: Publish refuses");
        // permission recorded: the refusal goes, the credit says so
        ctx.noted.insert("https://gamebanana.com/mods/99".into(), Noted { granted: true, note: "asked on Discord".into(), ..Default::default() });
        let assets = vec![resolve("models/forbidden.mdl", "", false, &ctx)];
        let (credits, warnings, blockers) = for_publish(&assets);
        assert!(blockers.is_empty() && warnings.is_empty());
        assert_eq!((credits[0].permission.as_str(), credits[0].note.as_str()), ("granted", "asked on Discord"));
        let readme = readme_section(&credits);
        assert!(readme.contains("Used with permission (asked on Discord)") && readme.contains("models/forbidden.mdl"));
        let rows = gamebanana_rows(&credits);
        assert_eq!(rows[0]["name"], "DimeX");
    }

    #[test]
    fn credits_are_cleaned_for_the_index() {
        let bad = vec![AssetCredit { files: vec!["models/a.mdl".into(), "../evil".into()], page: "https://evil.example".into(), name: "A\nB".into(),
            authors: vec![Author { name: "X".into(), url: "javascript:1".into(), ..Default::default() }, Author::default()], permission: "maybe".into(), note: "n".into() },
            AssetCredit { files: vec!["/abs".into()], ..Default::default() }];
        let c = clean_credits(bad);
        assert_eq!(c.len(), 1, "an entry with no good file goes");
        assert_eq!((c[0].files.len(), c[0].page.as_str(), c[0].name.as_str(), c[0].permission.as_str()), (1, "", "A B", ""));
        assert_eq!((c[0].authors.len(), c[0].authors[0].url.as_str()), (1, ""));
    }

    #[test]
    fn noting_permissions_and_credits() {
        let d = std::env::temp_dir().join(format!("ls-credits-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        assert!(note(&d, "https://evil.example/x", true, "x", vec![], "").is_err());
        assert!(note(&d, "https://gamebanana.com/mods/99", true, "", vec![], "").unwrap_err().contains("how you got permission"));
        note(&d, "https://gamebanana.com/mods/99", true, "asked on Discord, 2026-11-10", vec![], "").unwrap();
        let sha = format!("sha:{}", "a".repeat(64));
        note(&d, &sha, false, "", vec![Author { name: "Pal".into(), ..Default::default() }], "https://gamebanana.com/mods/5").unwrap();
        let all = load_noted(&d);
        assert!(all["https://gamebanana.com/mods/99"].granted);
        assert_eq!(all[&sha].authors[0].name, "Pal");
        note(&d, "https://gamebanana.com/mods/99", false, "", vec![], "").unwrap();
        assert!(!load_noted(&d).contains_key("https://gamebanana.com/mods/99"), "cleared");
        let _ = std::fs::remove_dir_all(&d);
    }
}
